// Appels à l'app (commandes Tauri de src/ui.rs) et ses nouvelles (événements).
// L'API de Tauri est exposée en global (`withGlobalTauri`).


type Tauri = {
  core: { invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> }
  event: { listen<T>(name: string, cb: (e: { payload: T }) => void): Promise<() => void> }
  window: { getCurrentWindow(): { minimize(): Promise<void>; toggleMaximize(): Promise<void>; close(): Promise<void> } }
}

import { mockTauri } from './mock'
import { i18n, t } from './i18n/index.svelte'

// Dans un navigateur (développement), une imitation de l'app.
const tauri: Tauri =
  (window as unknown as { __TAURI__?: Tauri }).__TAURI__ ?? (import.meta.env.DEV ? (mockTauri() as unknown as Tauri) : (undefined as never))

export const invoke = <T>(cmd: string, args?: Record<string, unknown>) => tauri.core.invoke<T>(cmd, args)
export const listen = <T>(name: string, cb: (payload: T) => void) => tauri.event.listen<T>(name, (e) => cb(e.payload))
export const currentWindow = () => tauri.window.getCurrentWindow()

export type Settings = {
  profile: 'jeu' | 'bureau' | 'max'
  codec: 'h264' | 'hevc' | 'av1' | null
  yuv444: boolean | null
  mbps: number | null
  reliable: boolean | null
  max_4k: boolean
  /** Écran virtuel sur mesure ; sinon comme cet écran. */
  custom: { width: number; height: number; refresh: number } | null
  /** Écrans de ce PC pris par la session : Auto (tous en Bureau et Max, un seul en Jeu et en Personnalisé), Cet écran, Tous les écrans. */
  displays: 'auto' | 'this' | 'all'
  vsync: boolean
  mouse: 'auto' | 'bureau' | 'jeu'
  gamepad: 'xbox360' | 'fidele'
  clipboard: boolean
  audio: 'auto' | '2' | '6' | '8' | '0'
  virtual_display: boolean
  /** Repris par les réglages de l'app (`AppInfo.battery`) ; gardé pour l'aller-retour. */
  battery: 'alleger' | 'garder'
  keep_last_image: boolean
  auto_reconnect: boolean
}

/** Mise à jour de la Station, telle qu'Admin la connaît (`installable` : depuis l'app ; `requires_setup` : avec l'installateur, sur l'ordinateur). */
export type StationUpdate = { available: boolean; version: string | null; installable: boolean; requires_setup: boolean; installing: boolean }

/** La Station de l'ordinateur (rubrique « Station » de ses réglages). */
export type StationInfo = { version: string; update: StationUpdate; restarting: boolean }

/**
 * Refus de `station_restart` et `station_update` : `in_session` (redemander avec `force` coupe la
 * session de `device`), `too_soon` (attendre `seconds`), `nothing_to_install`, `requires_setup`,
 * `busy` (installation ou redémarrage en cours), `unsupported` (Station 0.4.9 ou plus ancienne), `other`.
 */
export type StationRefusal = {
  kind: 'in_session' | 'too_soon' | 'nothing_to_install' | 'requires_setup' | 'busy' | 'unsupported' | 'other'
  message: string
  device: string | null
  seconds: number | null
}

/**
 * Une carte réseau de l'ordinateur pour le réveiller par le réseau (commande `wake`) : `ipv4`, sa
 * dernière adresse ; `broadcast`, la diffusion de son sous-réseau ; `armed`, Windows la laisse
 * réveiller l'ordinateur (`null` : inconnu) ; `name`, son nom (vide s'il est inconnu).
 */
export type WakeTarget = {
  mac: string
  ipv4: string | null
  broadcast: string | null
  kind: 'ethernet' | 'wifi' | ''
  armed: boolean | null
  name: string
}

/**
 * `station` : nul avec une Station 0.4.9 ou plus ancienne (ni redémarrage ni mise à jour depuis l'app).
 * `wake` : ses cartes réseau pour le réveil, Ethernet d'abord (vide avant la Station 0.5.0).
 */
export type Info = {
  hostname: string
  os: string
  gpu: string | null
  av1: boolean
  /** Version d'Admin, donnée aussi par les Stations d'avant `station`. */
  admin_version?: string | null
  /** `max_screens` : écrans que la Station sait créer (absent : un seul, Station antérieure). */
  virtual_display: { available: boolean; max_screens?: number }
  station: StationInfo | null
  wake?: WakeTarget[]
  /** `own_device` : un compte créé pour cet appareil par l'appairage par code (Station 0.5.0 et suivantes). */
  account?: { own_device: boolean }
  /** KyberNow Share (Station 0.8.0 et suivantes) ; `enabled` faux : partage coupé sur l'ordinateur. Absent : Station antérieure. */
  share?: { available: boolean; enabled: boolean }
}

/**
 * Dernière session d'au moins 3 s (rubrique Statistiques) : ce qui a été transmis
 * (`width` à `mbps`) manque dans celles gardées par la 0.4.x, qui ont `label` à la place.
 */
export type LastSession = {
  median_ms: number | null
  p95_ms: number | null
  seconds: number
  /** Fin, en secondes depuis 1970. */
  at: number
  width?: number | null
  height?: number | null
  refresh?: number | null
  /** Écrans de la session (absent : un seul) ; `width` à `refresh` sont ceux de l'écran principal, `mbps` le débit total. */
  screens?: number | null
  codec?: string | null
  mbps?: number | null
  /** « Bureau · HEVC 4:4:4 », en français (0.4.x). */
  label?: string
}

export type Computer = {
  id: string
  name: string
  host: string
  port: number
  user: string
  fingerprint: string | null
  os: string
  gpu: string | null
  settings: Settings
  last: LastSession | null
  /** Ajouté avec un code : un compte propre à ce PC. */
  by_code?: boolean
  /**
   * Ses cartes réseau gardées pour le réveiller une fois éteint (commande `wake`) : celles du
   * dernier `info` qui en donnait ; vide sans Station 0.5.0 vue allumée.
   */
  wake: WakeTarget[]
  online: boolean | null
  info: Info | null
  preview: string | null
  /** last : dernière image de votre session ; preview : aperçu de son écran. */
  preview_source: 'last' | 'preview' | null
  /** Réveil par le réseau envoyé : l'app attend l'ordinateur (2 min au plus), toutes les 2 s. */
  waking?: boolean
  /** La Station redémarre ou s'installe : l'app attend son retour (`station-outcome` à la fin). */
  station_pending?: 'restart' | 'update' | null
}

export type Found = { id: string; name: string; host: string; port: number; fingerprint: string | null }

export type Candidate = { host: string; port: number; name: string | null; fingerprint: string | null }

export type ThisPc = {
  name: string
  gpu: string
  h264: boolean
  hevc: boolean
  hevc444: boolean
  av1: boolean
  width: number
  height: number
  refresh: number
  audio_channels: number
  os: string
  on_battery: boolean
}

export type Update = { version: string; channel: 'stable' | 'beta'; notes: string; page: string; file: string; size: number }

/** Sur batterie : alléger (30 Hz au plus) ou garder, pour toutes les sessions de ce PC. */
export type Battery = 'alleger' | 'garder'

/**
 * `language` : choix des réglages ; `lang` : langue appliquée. `open_at_login` : « Ouvrir à
 * l'ouverture de session », tel que Windows le fera (décoché si on l'a désactivé dans le
 * Gestionnaire des tâches).
 */
export type AppInfo = {
  version: string
  channel: 'stable' | 'beta'
  language: 'auto' | 'fr' | 'en'
  lang: 'fr' | 'en'
  update: Update | null
  arch: string
  battery: Battery
  open_at_login: boolean
}

/**
 * Ce que l'ouverture demande (lien, barre des tâches, notification, installateur, menu de
 * l'icône près de l'horloge) : `show`, l'accueil sur cet ordinateur ; `settings`, ses réglages
 * (sur la rubrique `rubric`) ; `app`, les réglages de l'app (`check` : en cherchant une mise à jour).
 */
export type Action =
  | { kind: 'connect'; id: string }
  | { kind: 'add' }
  /** « Rejoindre avec un code » (menu de l'icône) ; `link` : `kybernow://join?…` (lien ou QR code d'un partage). */
  | { kind: 'join'; link: string | null }
  | { kind: 'link'; link: string }
  | { kind: 'updated' }
  | { kind: 'open' }
  | { kind: 'show'; id: string }
  | { kind: 'settings'; id: string; rubric: string | null }
  | { kind: 'app'; check: boolean }

/**
 * Événement `station-outcome` : fin du suivi d'un redémarrage ou d'une mise à jour de la Station,
 * lancés depuis les réglages ou le menu de l'icône. `silent` : revenue sans être partie (rien à dire).
 */
export type StationOutcome = { id: string; outcome: 'restarted' | 'updated' | 'update_failed' | 'slow_back' | 'silent'; version: string | null }

/** `sessions` : les ordinateurs en session, dans l'ordre d'ouverture (plusieurs ordinateurs, quatre au plus). */
export type Snapshot = { computers: Computer[]; discovered: Found[]; this_pc: ThisPc | null; sessions: string[]; version: string }

/** Plusieurs ordinateurs à la fois (docs/plusieurs-ordinateurs.md) : quatre au plus en même temps. */
export const MAX_SESSIONS = 4

export type Phase = { id: string; phase: 'preparing' | 'connecting' | 'first_images' | 'streaming' | 'reconnecting' | 'ending' | 'ended'; detail: string | null }

/**
 * Événement `session-station-restarting` : dernier recours de la connexion,
 * la Station redémarre (l'image n'arrivait pas, ou Kyber ne répondait pas).
 * L'écran de connexion affiche « La Station redémarre… » jusqu'au prochain
 * `session-status` (`preparing` si elle revient, `ending` puis `ended`
 * sinon). Il suit toujours un `session-status` à `connecting`, l'étape
 * montrée tant qu'on ne l'écoute pas.
 */
export type StationRestarting = { id: string }

export type Summary = { error: string | null; median_ms: number | null; p95_ms: number | null; seconds: number }

export type Stats = {
  latency_ms: number | null
  fps: number
  /** Définition de l'écran principal. */
  width: number
  height: number
  decoder: string
  relative: boolean
  rtt_ms: number | null
  audio_ms: number | null
  audio_gaps: number | null
  /** Écrans de la session. */
  screens: number
}

/** Événement `stats` de la barre : les mesures de la session `id` (une seule barre, elle garde celles de la session qu'elle sert). */
export type SessionStats = Stats & { id: string }

/**
 * Un ordinateur du menu « Ordinateurs » de la barre (`bar_computers`, événement `bar-computers`) :
 * son état et son point de couleur comme dans le menu de l'icône, et ce que fait un clic
 * (`show` : en session, sa fenêtre passe devant ; `connect` : sa session s'ouvre, en fenêtre ;
 * `full` : quatre ordinateurs sont déjà en session ; `wake`, `waking`, `retry` : comme l'accueil ;
 * `busy` : la Station redémarre ou s'installe).
 */
export type BarComputer = {
  id: string
  name: string
  state: 'online' | 'offline' | 'searching'
  dot: 'ok' | 'bad' | 'warn' | 'faint'
  pick: 'show' | 'connect' | 'full' | 'wake' | 'waking' | 'retry' | 'busy'
}

/** Événement `session-keep-sound` : « Garder le son » de la session `id`. */
export type KeepSound = { id: string; on: boolean }

/**
 * `connexion_state` : ce que montre la fenêtre de connexion d'une session suivante (plusieurs
 * ordinateurs) à son ouverture ; ensuite, les mêmes événements que l'accueil. `error` : la
 * session s'est terminée avant sa première image, avec cette raison.
 */
export type ConnexionView = {
  id: string
  phase: Phase['phase']
  restarting: boolean
  error: string | null
  /** Session d'invité (KyberNow Share) : « Invité sur {nom} », la teinte de l'ordinateur qui partage ; la raison d'un échec se lit telle quelle. */
  title?: string | null
  fingerprint?: string | null
  guest?: boolean
}

/**
 * Réponse de `displays_info` : le réglage « Écrans » tel que le cœur le décide (`enabled` : il se
 * change, sinon il est inactif sur « Cet écran » ; `shown` : sa valeur montrée), le nombre
 * d'écrans que la session prendra (1 : un seul) et si l'écran virtuel servira (`virtual_display` :
 * demandé et utilisable sur la Station ; sinon « Les écrans de l'ordinateur, tels quels »). Le
 * même calcul que la session.
 */
export type DisplaysInfo = { enabled: boolean; shown: Settings['displays']; count: number; virtual_display: boolean }

/**
 * Réponse de `connect` : les écrans que la session prend (« Préparation des écrans ») ;
 * `windowed` : une session suivante (plusieurs ordinateurs), qui s'ouvre dans sa fenêtre avec sa
 * propre fenêtre de connexion, sans passage sur l'accueil, ou un ordinateur déjà en session, dont
 * la fenêtre passe devant.
 */
export type ConnectInfo = { screens: number; windowed: boolean }

/** Événements `session-screens` (écrans de la session) et `session-replan` (la connexion reprend en session, avec ce nombre d'écrans). */
export type ScreensEvent = { id: string; screens: number }

/** `fond_state` et événement `fond-state` : ce que montre le fond de la connexion d'un écran (la ligne d'un écran que l'ordinateur n'a pas pu créer). */
export type FondState = { not_created: boolean }

// KyberNow Share (docs/kybernow-share.md, textes de docs/link-ui.md « Partage »).

/**
 * Le rôle d'un invité, deux droits indépendants que l'hôte donne ou reprend chacun sans toucher à
 * l'autre : `spectator` (aucun droit : voit et entend), `player` (ses manettes), `kbm` (le clavier et
 * la souris), `player_kbm` (les deux).
 */
export type ShareRole = 'spectator' | 'player' | 'kbm' | 'player_kbm'

/** Ses manettes partent vers l'ordinateur. */
export const rolePads = (r: ShareRole) => r === 'player' || r === 'player_kbm'
/** Son clavier et sa souris partent vers l'ordinateur. */
export const roleKbm = (r: ShareRole) => r === 'kbm' || r === 'player_kbm'
/** Un droit au moins : la barre dit « Joueur » (« Spectateur » sinon). */
export const rolePlays = (r: ShareRole) => r !== 'spectator'
/** Le rôle de ces deux droits. */
export const roleOf = (pads: boolean, kbm: boolean): ShareRole => (pads ? (kbm ? 'player_kbm' : 'player') : kbm ? 'kbm' : 'spectator')

/**
 * Un invité de la liste de l'hôte. `app` : `mac`, `windows`, `linux`, `ios`, `tvos`, `android`, `webos`
 * ou `web` ; `since` : son arrivée, en secondes Unix de la Station.
 */
export type ShareGuest = { id: string; name: string; app: string; role: ShareRole; since: number | null }

/** Un QR code : `size` modules de côté, `path` le tracé SVG de ses modules sombres (sans marge). */
export type ShareQr = { size: number; path: string }

/**
 * Le panneau de l'hôte (`bar-shown`, événement `share-state`, réponses des actions) : ce que « Partager »
 * peut faire (`availability` : `unsupported` Station trop ancienne, `disabled` partage coupé, `unknown`
 * l'ordinateur n'a rien dit encore ; `text` : le texte du cœur quand « Partager » est inactif), puis le
 * partage ouvert : `code` « 123 456 », `link` (apps, le QR code) et `web` (« Copier le lien »), l'écran
 * partagé, les invités, `full` (quatre invités).
 */
export type Share = {
  availability: 'available' | 'unsupported' | 'disabled' | 'unknown'
  text: string | null
  open: boolean
  code: string
  link: string
  web: string
  display: string | null
  guests: ShareGuest[]
  full: boolean
  qr: ShareQr | null
}

/** L'invité que la barre sert (barre réduite) : son rôle et l'hôte du partage. */
export type GuestBar = { role: ShareRole; host: string }

/** Réponse de `share_join` : la session d'invité à ouvrir (`share_connect`), son titre (« Invité sur Atelier ») et la teinte du passage. */
export type ShareJoined = { id: string; title: string; host: string; role: ShareRole; fingerprint: string | null }

/** Événements `share-role` (le rôle de l'invité de la session `id` a changé) et `share-state` (le panneau de l'hôte de la session `id`). */
export type ShareRoleEvent = { id: string; role: ShareRole }
export type ShareStateEvent = { id: string; share: Share }

/** Une pseudo-fiche d'ordinateur pour le passage et la connexion d'une session d'invité (ni dans la liste, ni gardée). */
export const guestComputer = (j: { title: string; fingerprint: string | null }) => ({ name: j.title, preview: null, fingerprint: j.fingerprint })

/**
 * « moins d'une minute », « 3 min », « 1 h 12 » : depuis combien de temps un invité est là
 * (docs/link-ui.md, « Durée »). `since` : son arrivée en secondes Unix de la Station, `now` :
 * l'heure de ce PC en secondes ; jamais négative (horloges décalées).
 */
export const sinceText = (since: number, now = Date.now() / 1000) => {
  const s = Math.max(0, Math.floor(now - since))
  return s < 60 ? t('share.under_minute') : duration(s)
}

/** « 8,9 » en français, « 8.9 » en anglais. */
export const ms = (v: number | null | undefined, digits = 1) => {
  if (v == null) return '—'
  const s = v.toFixed(digits)
  return i18n.lang === 'fr' ? s.replace('.', ',') : s
}

/** « 42 min », « 1 h 05 » (une minute au moins, comme sur Mac). */
export const duration = (s: number) => {
  const h = Math.floor(s / 3600)
  const m = Math.floor((s % 3600) / 60)
  return h > 0 ? t('time.hours', { h, m: String(m).padStart(2, '0') }) : t('time.minutes', { m: Math.max(1, m) })
}

/** Quand : « à l'instant », « aujourd'hui, 14 h 05 », « hier, 14 h 05 », « il y a 3 jours », « le 3 octobre ». */
export function moment(at: number, now = Date.now()) {
  const date = new Date(at * 1000)
  const locale = i18n.lang === 'fr' ? 'fr-FR' : 'en-GB'
  const time =
    i18n.lang === 'fr'
      ? `${String(date.getHours()).padStart(2, '0')}\u00a0h\u00a0${String(date.getMinutes()).padStart(2, '0')}`
      : date.toLocaleTimeString(locale, { hour: '2-digit', minute: '2-digit' })
  const day = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime()
  const days = Math.round((day(new Date(now)) - day(date)) / 86_400_000)
  if (days <= 0) return now - date.getTime() < 120_000 ? t('time.just_now') : t('time.today', { time })
  if (days === 1) return t('time.yesterday', { time })
  if (days < 7) return t('time.days_ago', { days })
  return t('time.on_day', { day: date.toLocaleDateString(locale, { day: 'numeric', month: 'long' }) })
}

/**
 * Un profil retouché (format, couleurs, débit, transport) est « Personnalisé ». Un format
 * choisi égal à celui du profil n'en est pas un écart (comme `plan` de plan.rs).
 */
export const isCustom = (s: Settings, pc: ThisPc | null) =>
  (s.codec != null && s.codec !== profileBase(s.profile, pc).codec) || s.yuv444 != null || s.mbps != null || s.reliable != null

/** Les quatre premiers groupes de l'empreinte d'Admin, comme Admin les montre (« 7C41 E09B AABB CCDD »). */
export const fingerprintGroups = (fp: string | null) =>
  (fp ?? '').replace(/^sha256:/, '').replaceAll(':', '').toUpperCase().match(/.{1,4}/g) ?? []

/** Valeurs d'un profil pour ce PC, comme le cœur les calcule (`StreamSettings::for_profile`, profile.rs). */
export function profileBase(profile: Settings['profile'], pc: ThisPc | null) {
  const codec: 'h264' | 'hevc' = (pc?.hevc ?? true) ? 'hevc' : 'h264'
  let [yuv444, mbps, reliable] = { jeu: [false, 60, false], bureau: [true, 90, true], max: [true, 300, false] }[profile] as [boolean, number, boolean]
  if (yuv444 && !(codec === 'hevc' && pc?.hevc444)) {
    yuv444 = false
    mbps += Math.floor(mbps / 3)
  }
  if (codec === 'h264') mbps += Math.floor(mbps / 2)
  return { codec, yuv444, mbps, reliable }
}

type Mode = { width: number; height: number; refresh: number }

/** `fit` de plan.rs : réduit pour tenir dans `maxW` × `maxH`, proportions gardées, dimensions paires (arrondi vers le bas). */
function fit(m: Mode, maxW: number, maxH: number): Mode {
  if (m.width <= maxW && m.height <= maxH) return m
  const k = Math.min(maxW / m.width, maxH / m.height)
  return { width: Math.floor(m.width * k) & ~1, height: Math.floor(m.height * k) & ~1, refresh: m.refresh }
}

/**
 * Ce que la session demandera à l'ordinateur : `session_plan` (ui.rs) et
 * `plan` (plan.rs), règle pour règle. Mode de l'écran virtuel et débit en
 * Mbit/s ; `null` sans écran connu (ni ce PC ni taille sur mesure).
 * `battery` : « Sur batterie » des réglages de l'app.
 */
export function sessionPlan(s: Settings, pc: ThisPc | null, battery: Battery): (Mode & { mbps: number }) | null {
  const custom = s.custom && s.custom.width >= 640 && s.custom.height >= 360 && s.custom.refresh >= 24 ? s.custom : null
  let mode: Mode | null = custom ?? (pc ? { width: pc.width, height: pc.height, refresh: pc.refresh } : null)
  if (!mode) return null
  if (s.max_4k && !s.custom) mode = fit(mode, 3840, 2160)
  if (battery === 'alleger' && pc?.on_battery) mode = { ...mode, refresh: Math.min(30, mode.refresh) }
  const base = profileBase(s.profile, pc)
  // NVENC n'encode pas l'H.264 au-delà de 4096 de large.
  if ((s.codec ?? base.codec) === 'h264') mode = fit(mode, 4096, 4096)
  // Débit du profil pour ce mode (`StreamSettings::bitrate`), sauf débit réglé.
  const ratio = (mode.width * mode.height * Math.max(1, mode.refresh)) / (3840 * 2160 * 60)
  const scale = Math.min(3, Math.max(0.4, Math.pow(ratio, 0.75)))
  const mbps = s.mbps ?? Math.min(4294, Math.max(5, Math.round(base.mbps * scale)))
  return { ...mode, mbps }
}

/** Noms des profils tels que le côté Rust les écrit (« Bureau · HEVC 4:4:4 » : barre, et dernière session de la 0.4.x). */
const profileNames: Record<string, string> = { Jeu: 'jeu', Bureau: 'bureau', Max: 'max', Personnalisé: 'custom' }

/** « Bureau · HEVC 4:4:4 » dans la langue de l'interface. */
export const localLabel = (label: string) => {
  const [first, ...rest] = label.split(' · ')
  const key = profileNames[first]
  return key ? [t(`profiles.${key}`), ...rest].join(' · ') : label
}

/** Teinte de la signature d'un ordinateur, tirée de l'empreinte de son Admin. */
export const hue = (fp: string | null, salt = 0) => {
  let h = 2166136261 + salt
  for (const c of fp ?? 'kybernow') h = Math.imul(h ^ c.charCodeAt(0), 16777619)
  return Math.abs(h) % 360
}
