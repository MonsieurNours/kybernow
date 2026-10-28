// Client de l'API kyber-admin.

import { langHeaders, t } from './i18n/index.svelte'

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

export type LaunchKind = 'app' | 'uri' | 'path'

/** App ouverte sur l'ordinateur au début de chaque session d'un appareil. */
export interface Launch {
  kind: LaunchKind
  /** Identifiant d'application Windows, adresse (`steam://…`) ou exécutable. */
  target: string
  name: string
  /** Fichier d'icône du catalogue (`/api/apps/icon/<nom>`). */
  icon?: string | null
  exe?: string | null
  /** Adresse qui ferme l'app (`steam://close/bigpicture`). */
  close?: string | null
  close_at_end: boolean
  skip_desktop: boolean
}

/** L'app sait-elle se fermer proprement à la fin de la session ? (Même règle que le serveur.) */
export function canCloseLaunch(l: Launch): boolean {
  return !!l.close || !!l.exe || l.kind !== 'uri'
}

export interface AppEntry {
  id: string
  name: string
  icon?: string | null
  exe?: string | null
}

export interface Suggestion {
  id: string
  name: string
  kind: LaunchKind
  target: string
  close?: string | null
  exe?: string | null
  icon?: string | null
}

export interface AppRoot {
  /** `programs`, `programs86`, `desktop`, `userPrograms` */
  id: string
  path: string
}

export interface Catalogue {
  refreshed_at: number
  user: string | null
  apps: AppEntry[]
  suggestions: Suggestion[]
  roots: AppRoot[]
  error?: string | null
}

export interface BrowseEntry {
  name: string
  path: string
}

export interface Browse {
  roots?: AppRoot[]
  path?: string
  root?: string
  parent?: string | null
  folders?: BrowseEntry[]
  files?: BrowseEntry[]
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
  /** App ouverte au début de ses sessions, si réglée. */
  launch?: Launch | null
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
  /** Modes des écrans d'une session à plusieurs écrans (le principal en premier). */
  screens?: ModeInfo[]
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
  /** Cartes réseau pour le réveil par le réseau, Ethernet d'abord (absent avant la 0.5.0). */
  wake?: WakeCard[]
  /** Partage en cours (KyberNow Share), qu'il vienne d'un appareil ou du PC (absent avant la 0.8.0). */
  share?: ShareView | null
  /** Faux si le partage est coupé (`network.multi_client`) ; absent avant la 0.8.0. */
  share_enabled?: boolean
}

/** Droits d'un invité : il regarde, il joue (ses manettes), ou il a aussi le clavier et la souris. */
export type ShareRole = 'spectator' | 'player' | 'kbm' | 'player_kbm'

export interface ShareGuest {
  id: string
  name: string
  /** App qui a rejoint : `mac`, `windows`, `linux`, `ios`, `tvos`, `android`, `webos` ou `web`. */
  app: string
  device: string
  role: ShareRole
  since: number
  last_beat: number
  /** Son image (correctif Kyber 0020) ; absente d'une Station qui ne la connaît pas. */
  image?: GuestImage | null
  /**
   * Une demande venue par le relais, pas encore acceptée (docs/relais.md, « Partage ») : spectatrice,
   * `since` est l'heure de la demande ; elle ne compte pas dans les invités.
   */
  pending?: boolean
  /** Par où l'invité est venu : le réseau (`lan`) ou le relais (`relay`, « hors de chez vous »). */
  via?: 'lan' | 'relay'
}

/** « Qualité de l'image » d'un invité (docs/kybernow-share.md, « L'image d'un invité »). */
export type ShareQuality = 'auto' | 'host' | '2160p60' | '1080p60' | '720p30'

/**
 * L'image d'un invité (`image` de son signe de vie, réponse de `POST /api/share/quality`,
 * `guests[].image` de la vue de l'hôte) : la qualité demandée, puis ce que Kyber en a fait, `null`
 * tant qu'il ne le sait pas. `reason` : `asked` (ce qui est demandé), `capped` (bornée par l'image de
 * l'hôte), `limit` (pas d'image de plus : la plus proche, plus légère), `lag` (allégée pour suivre).
 */
export interface GuestImage {
  quality: ShareQuality
  width: number | null
  height: number | null
  fps: number | null
  codec: string | null
  follows_host: boolean | null
  reason: 'asked' | 'capped' | 'limit' | 'lag' | null
}

/** Un partage ouvert (KyberNow Share, docs/kybernow-share.md). */
export interface ShareView {
  open: true
  /** Depuis la session d'un appareil (`host`), ou depuis le PC lui-même (`local`). */
  origin: 'host' | 'local'
  /** Nom de l'appareil hôte, ou nom de l'ordinateur. */
  host: string
  /** Six chiffres. */
  code: string
  /** `kybernow://join?…` : pour les apps (QR code). */
  link: string
  /** Adresse du client web en invité : pour un navigateur (« Copier le lien ») ; le lien du relais quand il est joint. */
  web: string
  /** Adresse du client web sur le réseau, même quand `web` est le lien du relais. */
  web_local?: string
  /** Le relais du partage : `null` (pas de relais réglé), joint ou injoignable (absent avant le relais). */
  relay?: { state: 'connecting' | 'joined' | 'unreachable' } | null
  display: { id: number; index: number; label: string; width: number; height: number; refresh: number }
  guests: ShareGuest[]
  max_guests: number
  opened_at: number
}

/** Un écran physique du PC, à partager (`GET /api/local/displays`, l'écran principal d'abord). */
export interface LocalDisplay {
  id: number
  /** `\\.\DISPLAY1` */
  name: string
  index: number
  /** « Écran 1 », dans la langue de la requête. */
  label: string
  width: number
  height: number
  refresh: number
  primary: boolean
  x: number
  y: number
}

/** Les invités entrés : une demande venue par le relais n'en est pas un tant qu'elle n'est pas acceptée. */
export function joinedGuests(view: ShareView | null | undefined): ShareGuest[] {
  return view?.guests.filter((g) => !g.pending) ?? []
}

/** Les demandes venues par le relais, en attente de l'accord de l'hôte. */
export function shareRequests(view: ShareView | null | undefined): ShareGuest[] {
  return view?.guests.filter((g) => g.pending) ?? []
}

/**
 * Ce que le panneau de partage sait faire : les routes du PC (`/api/local/share…`,
 * Admin) ou celles de l'hôte (`/api/share…`, client web).
 */
export interface ShareOps {
  code(): Promise<ShareView>
  role(id: string, role: ShareRole): Promise<ShareView>
  remove(id: string): Promise<ShareView>
  /** Accepte ou refuse une demande venue par le relais. */
  accept(id: string): Promise<ShareView>
  refuse(id: string): Promise<ShareView>
  stop(): Promise<unknown>
}

/**
 * Le relais de KyberNow (`GET /api/relay`, docs/relais.md, « Admin ») : l'adresse du relais (vide :
 * pas de relais), « Accès hors de chez vous », l'identifiant de la Station, l'état de la connexion
 * (`reason` dans la langue d'Admin), le blocage après trop d'essais et les sessions par le relais.
 */
export interface RelayStatus {
  server: string
  remote_access: boolean
  id: string
  state: 'off' | 'connecting' | 'joined' | 'unreachable'
  reason: string | null
  transport: 'quic' | 'tcp' | null
  /** Accès par le relais bloqué jusqu'à cette heure (secondes Unix), après trop d'essais. */
  locked_until: number | null
  sessions: { device: string; since: number }[]
}

/**
 * Une carte réseau physique en service, pour le réveil par le réseau (`wake` de
 * `/api/status` et de `/api/stream/info`). `armed` : Windows permet à la carte de
 * réveiller l'ordinateur (`null` : inconnu).
 */
export interface WakeCard {
  mac: string
  ipv4: string | null
  broadcast: string | null
  kind: 'ethernet' | 'wifi'
  armed: boolean | null
  name: string
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
    /** Écrans virtuels de la session, à leur mode courant et à leur place (le principal en premier). */
    screens?: { width: number; height: number; refresh: number; x: number; y: number; primary?: boolean }[]
    screens_requested?: number
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
  /** La version s'installe avec l'installateur (`setup`). */
  requires_setup: boolean
  /** Installateur de la Station listé par le manifeste signé : KyberNow-Station-Setup.exe,
   *  ou KyberNow-Server-Setup.exe pour une version publiée avant le renommage. */
  setup?: string
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
  /**
   * `code` : raison stable d'un refus, quand Admin en donne une (`session`, `too_soon`…) ;
   * `retryAfter` : secondes à attendre avant de refaire la demande (429 avec `Retry-After`).
   */
  constructor(public status: number, message: string, public code?: string, public retryAfter?: number) {
    super(message)
  }
}

/** Appelé quand une requête renvoie 401 (session expirée). */
let onUnauthorized: () => void = () => {}
export function setUnauthorizedHandler(fn: () => void) {
  onUnauthorized = fn
}

async function request<T>(method: string, path: string, body?: unknown): Promise<T> {
  // La console répond dans la langue de l'interface (messages d'erreur, notes de version, fil).
  const headers: Record<string, string> = langHeaders()
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
    throw new ApiError(res.status, data?.error ?? t('common.error', { status: res.status }), typeof data?.code === 'string' ? data.code : undefined)
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
  /** Redémarre la Station entière (Kyber et Admin) : 409 (`code` : `session`) si un appareil est en session, sauf `force`. */
  stationRestart: (force = false) => request<{ ok: boolean }>('POST', '/api/station/restart', force ? { force } : undefined),
  audioSettings: (settings: AudioSettings) => request<AudioStatus>('PUT', '/api/audio/settings', settings),

  devices: () => request<{ devices: Device[] }>('GET', '/api/devices'),
  /** Règle (ou retire, avec `null`) l'app ouverte au début des sessions d'un appareil. */
  setLaunch: (id: string, launch: Launch | null) =>
    request<{ devices: Device[] }>('PUT', `/api/devices/${encodeURIComponent(id)}/launch`, { launch }),
  /** Catalogue des apps de l'ordinateur (lu à la première demande). */
  apps: () => request<Catalogue>('GET', '/api/apps'),
  appsRefresh: () => request<Catalogue>('POST', '/api/apps/refresh'),
  appsBrowse: (path?: string) => request<Browse>('GET', `/api/apps/browse${path ? `?path=${encodeURIComponent(path)}` : ''}`),
  appIconUrl: (name: string) => `/api/apps/icon/${encodeURIComponent(name)}`,
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

  // Partage depuis le PC (KyberNow Share) : les routes de l'icône, avec la session d'Admin.
  // Elles gèrent aussi le partage d'un appareil hôte (retirer un invité, arrêter).
  displays: () => request<LocalDisplay[]>('GET', '/api/local/displays'),
  /** Refus : `code` vaut `disabled` (partage coupé), `session` (un appareil est en session) ou `unknown_display`. */
  shareOpen: (display: number) => request<ShareView>('POST', '/api/local/share/open', { display }),
  share: () => request<ShareView | { open: false }>('GET', '/api/local/share'),
  shareCode: () => request<ShareView>('POST', '/api/local/share/code'),
  shareRole: (id: string, role: ShareRole) =>
    request<ShareView>('POST', `/api/local/share/guest/${encodeURIComponent(id)}`, { role }),
  shareRemove: (id: string) => request<ShareView>('DELETE', `/api/local/share/guest/${encodeURIComponent(id)}`),
  /** Une demande venue par le relais : `404` (`unknown_guest`) si elle n'est plus là, `409` (`full`) si les quatre places sont prises. */
  shareAccept: (id: string) => request<ShareView>('POST', `/api/local/share/guest/${encodeURIComponent(id)}/accept`),
  shareRefuse: (id: string) => request<ShareView>('POST', `/api/local/share/guest/${encodeURIComponent(id)}/refuse`),
  shareStop: () => request<{ open: false }>('DELETE', '/api/local/share'),

  // Relais de KyberNow (« Hors de chez vous ») : session d'Admin, jamais par le relais.
  relay: () => request<RelayStatus>('GET', '/api/relay'),
  /** `server` vide : pas de relais ; adresse invalide : `400` avec le texte d'Admin. */
  setRelay: (changes: { server?: string; remote_access?: boolean }) => request<RelayStatus>('PUT', '/api/relay', changes),
}

/** Le panneau de partage d'Admin : routes du PC. */
export const localShare: ShareOps = {
  code: api.shareCode,
  role: api.shareRole,
  remove: api.shareRemove,
  accept: api.shareAccept,
  refuse: api.shareRefuse,
  stop: api.shareStop,
}
