// Client de l'API kyber-admin.

export interface Choice {
  value: string
  label: string
  hint: string
}

export type Kind =
  | { type: 'bool' }
  | { type: 'int'; min: number; max: number }
  | { type: 'enum'; options: Choice[] }

export interface Field {
  id: string
  group: string
  kind: Kind
  default: unknown
  label: string
  help: string
  details: string
  advice: string
  caution?: string
  unit?: string
}

export interface Group {
  id: string
  label: string
  help: string
}

export interface FieldValue {
  id: string
  value: unknown
  explicit: boolean
}

export interface User {
  username: string
  insecure_default: boolean
}

export type ServiceState = 'running' | 'stopped' | 'starting' | 'stopping' | 'not_installed' | 'unknown'

export interface GpuDevice {
  name: string
  gpu_utilization?: number
  encoder_utilization?: number
  decoder_utilization?: number
  encoder_sessions?: number
  encoder_fps?: number
  encoder_latency_us?: number
  temperature_c?: number
  memory_used_mb?: number
  memory_total_mb?: number
  power_w?: number
  pstate?: number
}

export interface Status {
  hostname?: string
  lan_ip?: string
  admin_version: string
  kyber_dir: string
  config_path: string
  config_found: boolean
  service: { state: ServiceState; simulated: boolean }
  controller: { port: number; reachable: boolean }
  restart_pending: boolean
  gpu: { driver?: string; devices: GpuDevice[] } | null
}

export interface LogFile {
  name: string
  size: number
  modified: number
}

export type Trigger = 'off' | 'apps' | 'all'

export interface VdSettings {
  trigger: Trigger
  width: number
  height: number
  refresh: number
}

export interface DisplayStatus {
  supported: boolean
  settings: VdSettings
  driver: { available: boolean; info?: { version: string; watchdog_seconds: number }; error?: string }
  session: null | {
    origin: 'app' | 'auto' | 'test'
    mode: { width: number; height: number; refresh: number }
    user: string | null
    started_at: number
    seconds_left: number | null
    applied: { gdi_name?: string; width?: number; height?: number; refresh?: number; disabled_others?: number }
  }
  last_error: string | null
}

export type AudioChannels = 'auto' | 'stereo' | '5.1' | '7.1'

export interface AudioSettings {
  trigger: Trigger
  channels: AudioChannels
}

export interface AudioEndpoint {
  id: string
  name: string
  channels: number
}

export interface AudioStatus {
  supported: boolean
  settings: AudioSettings
  devices: { speakers?: AudioEndpoint | null; default?: AudioEndpoint | null; error?: string }
  session: null | { origin: 'app' | 'auto' | 'test'; channels: number; started_at: number }
  last_error: string | null
}

export interface UpdateStatus {
  current: string
  latest: string | null
  available: boolean
  notes: string | null
  published: string | null
  page: string | null
  installable: boolean
  checked_seconds_ago: number | null
  error: string | null
  installing: string | null
  repo: string
}

export interface Pairing {
  link: string
  hostname: string
  address: string
  port: number
  fingerprint: string | null
}

export class ApiError extends Error {
  constructor(public status: number, message: string) {
    super(message)
  }
}

/** Appelé quand une requête renvoie 401 (session expirée). */
let onUnauthorized: () => void = () => {}
export function setUnauthorizedHandler(fn: () => void) {
  onUnauthorized = fn
}

async function request<T>(method: string, path: string, body?: unknown): Promise<T> {
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
  if (!res.ok) {
    if (res.status === 401 && path !== '/api/login') onUnauthorized()
    throw new ApiError(res.status, data?.error ?? `Erreur ${res.status}`)
  }
  return data as T
}

export const api = {
  session: () =>
    request<{ configured: boolean; authenticated: boolean; setup_code_file: string }>('GET', '/api/session'),
  setup: (code: string, password: string) => request('POST', '/api/setup', { code, password }),
  login: (password: string) => request('POST', '/api/login', { password }),
  logout: () => request('POST', '/api/logout'),
  changePassword: (current: string, next: string) => request('POST', '/api/password', { current, new: next }),

  status: () => request<Status>('GET', '/api/status'),
  schema: () => request<{ groups: Group[]; fields: Field[] }>('GET', '/api/schema'),
  config: () => request<{ values: FieldValue[] }>('GET', '/api/config'),
  updateConfig: (changes: Record<string, unknown>) =>
    request<{ values: FieldValue[]; restart_pending: boolean }>('PUT', '/api/config', { changes }),
  rawConfig: () => request<{ text: string }>('GET', '/api/config/raw'),
  updateRawConfig: (text: string) => request('PUT', '/api/config/raw', { text }),

  users: () => request<{ users: User[] }>('GET', '/api/users'),
  upsertUser: (username: string, password: string) =>
    request<{ users: User[] }>('PUT', `/api/users/${encodeURIComponent(username)}`, { password }),
  deleteUser: (username: string) =>
    request<{ users: User[] }>('DELETE', `/api/users/${encodeURIComponent(username)}`),

  service: (action: 'start' | 'stop' | 'restart') =>
    request<{ state: ServiceState }>('POST', `/api/service/${action}`),

  display: () => request<DisplayStatus>('GET', '/api/display'),
  displaySettings: (settings: VdSettings) => request<DisplayStatus>('PUT', '/api/display/settings', settings),
  displayTest: (mode: { width: number; height: number; refresh: number }, seconds: number) =>
    request<DisplayStatus>('POST', '/api/display/test', { ...mode, seconds }),
  displayStop: () => request<DisplayStatus>('POST', '/api/display/stop'),
  audio: () => request<AudioStatus>('GET', '/api/audio'),
  pairing: () => request<Pairing>('GET', '/api/pairing'),
  update: () => request<UpdateStatus>('GET', '/api/update'),
  updateCheck: () => request<UpdateStatus>('POST', '/api/update/check'),
  updateInstall: () => request<{ ok: boolean; version: string }>('POST', '/api/update/install'),
  audioSettings: (settings: AudioSettings) => request<AudioStatus>('PUT', '/api/audio/settings', settings),

  logs: () => request<{ files: LogFile[] }>('GET', '/api/logs'),
  logTail: (name: string, lines: number) =>
    request<{ text: string }>('GET', `/api/logs/${encodeURIComponent(name)}?lines=${lines}`),
}
