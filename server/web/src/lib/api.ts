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
  /** Appareil appairé par code dont c'est l'accès propre. */
  device: string | null
}

export type ServiceState = 'running' | 'stopped' | 'starting' | 'stopping' | 'not_installed' | 'unknown'

export type GpuVendor = 'nvidia' | 'amd' | 'intel'

export interface GpuDevice {
  name: string
  vendor: GpuVendor
  driver?: string
  /** Un flux de Kyber est encodé par cette carte (toutes marques). */
  encoding: boolean
  gpu_utilization?: number
  encoder_utilization?: number
  decoder_utilization?: number
  /** NVIDIA seulement (NVML), comme les champs suivants. */
  encoder_sessions?: number
  encoder_fps?: number
  encoder_latency_us?: number
  temperature_c?: number
  memory_used_mb?: number
  memory_total_mb?: number
  power_w?: number
  pstate?: number
}

export interface ModeInfo {
  width: number
  height: number
  refresh: number
}

export interface Device {
  id: string
  name: string
  kind: string
  os: string | null
  app: string | null
  user: string
  first_seen: number
  last_seen: number
  sessions: number
  last_mode: ModeInfo | null
  latency_ms: number | null
  /** `user` est le compte propre de l'appareil (appairage par code). */
  own_account: boolean
}

/** Accès d'un appareil que Kyber en marche ne connaît pas encore (il redémarre dès qu'il est libre). */
export interface PendingAccess {
  account: string
  device: string
  /** Vrai : accès donné ; faux : accès retiré. */
  grant: boolean
}

/** Un appareil sans compte demande l'accès avec le code qu'il affiche. */
export interface PairRequest {
  id: string
  code: string
  name: string
  kind: string
  os: string | null
  app: string | null
  address: string
  expires_at: number
}

export interface LiveSession {
  device_id: string
  name: string
  kind: string
  user: string
  started_at: number
  mode: ModeInfo | null
  profile: string | null
  latency_ms: number | null
  end_requested: boolean
}

export interface LastSession {
  device_id: string
  name: string
  started_at: number
  ended_at: number
}

export interface ActivityEvent {
  at: number
  kind: 'info' | 'warn'
  text: string
}

export interface Status {
  hostname?: string
  os?: string | null
  lan_ip?: string
  admin_version: string
  kyber_dir: string
  config_path: string
  config_found: boolean
  service: { state: ServiceState; simulated: boolean }
  controller: { port: number; reachable: boolean }
  restart_pending: boolean
  pending_access: PendingAccess[]
  gpu: { driver?: string; devices: GpuDevice[] } | null
  activity: { live: LiveSession | null; last: LastSession | null }
  update_available: string | null
  pair_requests: PairRequest[]
}

export interface LogFile {
  name: string
  size: number
  modified: number
}

export type Trigger = 'off' | 'apps' | 'all'

/** Si le mode de l'écran virtuel change en session : laisser faire ou le remettre. */
export type ModePolicy = 'follow' | 'restore'

export interface VdSettings {
  trigger: Trigger
  width: number
  height: number
  refresh: number
  on_mode_change: ModePolicy
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
    mode_policy: ModePolicy
  }
  /** Mode courant de l'écran virtuel et écrans actifs, relus chaque seconde en session. */
  mode: { width: number; height: number; refresh: number } | null
  displays: number | null
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

export type UpdateChannel = 'stable' | 'beta'

export interface UpdateStatus {
  current: string
  /** Canal suivi : versions publiées, ou aussi les préversions. */
  channel: UpdateChannel
  latest: string | null
  available: boolean
  /** Préversion GitHub (canal bêta). */
  prerelease: boolean
  notes: string | null
  published: string | null
  page: string | null
  /** Manifeste signé vérifié, compatible avec cet ordinateur. */
  installable: boolean
  /** Pourquoi la version ne s'installe pas depuis la console (signature, canal, version). */
  verify_error: string | null
  /** La version s'installe avec KyberNow-Setup.exe. */
  requires_setup: boolean
  /** Version gardée dans les dossiers .prev (retour en arrière possible). */
  rollback: string | null
  checked_seconds_ago: number | null
  error: string | null
  installing: string | null
  /** Dernière installation arrêtée en route, avec la fin de son journal. */
  install_error?: string | null
  /** Échec écrit par l'applicateur : console muette après la mise à jour, version précédente remise. */
  last_failure?: { version: string; message: string; rolled_back: boolean; log: string } | null
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
    request<{ configured: boolean; authenticated: boolean; setup_code_file: string; hostname?: string | null }>('GET', '/api/session'),
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
  updateChannel: (channel: UpdateChannel) => request<UpdateStatus>('PUT', '/api/update', { channel }),
  /** 409 si un appareil est en session, sauf `force`. */
  updateInstall: (force = false) => request<{ ok: boolean; version: string }>('POST', '/api/update/install', force ? { force } : undefined),
  updateRollback: (force = false) => request<{ ok: boolean; version: string }>('POST', '/api/update/rollback', force ? { force } : undefined),
  audioSettings: (settings: AudioSettings) => request<AudioStatus>('PUT', '/api/audio/settings', settings),

  devices: () => request<{ devices: Device[] }>('GET', '/api/devices'),
  forgetDevice: (id: string) =>
    request<{ devices: Device[]; revoked: boolean; restart_pending: boolean; pending_access: PendingAccess[] }>('DELETE', `/api/devices/${encodeURIComponent(id)}`),
  pairAccept: (id: string) =>
    request<{
      username: string
      restarted: boolean
      restart_pending: boolean
      pending_access: PendingAccess[]
      pair_requests: PairRequest[]
      devices: Device[]
    }>(
      'POST',
      `/api/pair/accept/${encodeURIComponent(id)}`,
    ),
  pairRefuse: (id: string) => request<{ pair_requests: PairRequest[] }>('POST', `/api/pair/refuse/${encodeURIComponent(id)}`),
  events: (limit = 50) => request<{ events: ActivityEvent[] }>('GET', `/api/events?limit=${limit}`),
  endSession: () => request<{ ok: boolean; method: 'app' | 'restart' | 'none' }>('POST', '/api/session/end'),
  /** Adresse de l'aperçu de l'écran (JPEG). */
  previewUrl: (width: number, bust: number) => `/api/preview?width=${width}&t=${bust}`,

  logs: () => request<{ files: LogFile[] }>('GET', '/api/logs'),
  logTail: (name: string, lines: number) =>
    request<{ text: string }>('GET', `/api/logs/${encodeURIComponent(name)}?lines=${lines}`),
}
