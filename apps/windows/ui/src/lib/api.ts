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
  /** Prêt de l'ordinateur (Station 0.9.0 et suivantes) ; `enabled` faux : prêt coupé dans Admin. Absent : Station antérieure. */
  lend?: { available: boolean; enabled: boolean }
  /** Le relais de la Station (Station 0.8.0 et suivantes, relais réglé) : gardé pour chaque ordinateur (`Computer.relay`). */
  relay?: RelayInfo | null
}

/**
 * Le relais d'un ordinateur (`info.relay`) : le relais (`share.kybernow.app`), son identifiant chez lui, et
 * « Accès hors de chez vous » actif dans Admin. L'app en fait la route pour l'ouvrir hors de chez soi
 * (docs/relais.md).
 */
export type RelayInfo = { server: string; id: string; remote_access: boolean }

/** `transport` d'une session : sur le réseau, par le relais (QUIC), par le relais en repli TCP (l'UDP ne passe pas). */
export type Transport = 'direct' | 'relay' | 'relay_tcp'

/** Le chemin gardé avec la dernière session (`LastSession.route`) : ligne « Chemin » des statistiques. Allers-retours en ms. */
export type RouteInfo = { transport: Transport; relay?: string | null; rtt_client?: number | null; rtt_station?: number | null }

/**
 * Par où passe une session (événement `session-route`, et dans `bar-shown`) : la puce « Par le relais » de la
 * barre (`via_relay`), et `label` (« Direct », « Par le relais », « Par le relais (TCP) ») dans la langue de l'app.
 */
export type RouteView = { transport: Transport; label: string; via_relay: boolean; rtt_client: number | null; rtt_station: number | null }

/** Événement `session-route` : le chemin de la session `id` a changé. */
export type RouteEvent = { id: string; route: RouteView }

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
  /** Par où est passée la session : ligne « Chemin » (absent des sessions d'avant le relais). */
  route?: RouteInfo | null
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
  /** Son relais, gardé du dernier `info` (absent d'une Station antérieure ou sans relais réglé). */
  relay?: RelayInfo | null
  online: boolean | null
  /**
   * Joignable seulement par le relais : l'état est « Hors de chez vous » (`online` est alors vrai : on peut s'y
   * connecter).
   */
  away?: boolean
  /**
   * Par le relais : `away`, ou une session de cet ordinateur y passe. Admin refuse alors le redémarrage et la mise à
   * jour de la Station : la rubrique Station les désactive, avec la raison.
   */
  via_relay?: boolean
  info: Info | null
  preview: string | null
  /** last : dernière image de votre session ; preview : aperçu de son écran. */
  preview_source: 'last' | 'preview' | null
  /** Réveil par le réseau envoyé : l'app attend l'ordinateur (2 min au plus), toutes les 2 s. */
  waking?: boolean
  /** La Station redémarre ou s'installe : l'app attend son retour (`station-outcome` à la fin). */
  station_pending?: 'restart' | 'update' | null
  /** Prêté à un ami : « Prêté à {prénom} », « Reprendre l'ordinateur » (absent : rien de prêté). */
  lend?: HomeLend | null
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
  /** Le lien d'invitation d'un compte d'ami (`kybernow://borrow?…`) : « Ordinateur de {hôte} » à ajouter. */
  | { kind: 'borrow'; link: string }
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
  state: 'online' | 'away' | 'offline' | 'searching'
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
  /** Session d'emprunt (prêt de l'ordinateur) : « Préparation de l'ordinateur » en première étape. */
  borrow?: boolean
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
 * La « Qualité de l'image » d'un invité : `auto` (proposée : l'écran de ce PC et ce que son décodeur
 * suit), `host` (« Comme l'hôte »), `2160p60` (« 4K · 60 Hz »), `1080p60`, `720p30`. L'image d'un invité
 * n'est jamais au-dessus de celle de l'hôte. Retenue dans `app.json` pour la fois suivante.
 */
export type ShareQuality = 'auto' | 'host' | '2160p60' | '1080p60' | '720p30'

/** Dans l'ordre du choix. */
export const shareQualities: ShareQuality[] = ['auto', 'host', '2160p60', '1080p60', '720p30']

/**
 * Un invité de la liste de l'hôte. `app` : `mac`, `windows`, `linux`, `ios`, `tvos`, `android`, `webos`
 * ou `web` ; `since` : son arrivée, en secondes Unix de la Station ; `image` : sa qualité d'image
 * (« 1080p · 60 Hz »), seulement quand ce n'est pas celle de l'hôte.
 */
export type ShareGuest = { id: string; name: string; app: string; role: ShareRole; since: number | null; image: string | null; away: boolean }

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
  /** Les invités qui ont une place (une demande n'en prend pas). */
  guests: ShareGuest[]
  /** Les demandes venues par le relais, qui attendent « Accepter » ou « Refuser ». */
  requests: ShareGuest[]
  /**
   * Le relais du partage : `joined`, `connecting` (lien du relais en préparation : « Préparation du lien hors de
   * chez vous… », copie et QR code retenus 10 s au plus), `unreachable` (« Pas encore joignable hors de chez vous. »),
   * `null` sans relais.
   */
  relay: RelayState | null
  full: boolean
  qr: ShareQr | null
}

/** Le relais d'un code (partage, prêt) : tenu (`joined`), lien du relais en préparation (`connecting`), injoignable. */
export type RelayState = 'joined' | 'connecting' | 'unreachable'

/**
 * L'invité que la barre sert (barre réduite) : son rôle, l'hôte du partage, la « Qualité de l'image »
 * demandée (`quality`) et la qualité reçue (`image`, « 1080p · 60 Hz » ; `null` tant que Kyber ne la
 * connaît pas : la barre ne dit alors que la qualité demandée).
 */
export type GuestBar = { role: ShareRole; host: string; quality: ShareQuality; image: string | null }

/** Réponse de `share_join` quand l'invité entre : la session d'invité à ouvrir (`share_connect`), son titre (« Invité sur Atelier ») et la teinte du passage. */
export type ShareJoined = { id: string; title: string; host: string; role: ShareRole; fingerprint: string | null }

/**
 * Réponse de `share_join` : l'invité entre (`joined`), ou sa demande attend l'accord de l'hôte (`pending`, par le
 * relais) : l'écran « Rejoindre » montre « En attente de l'accord de l'hôte… » et rappelle `share_wait` avec ce numéro.
 */
export type ShareJoinOutcome = ({ kind: 'joined' } & ShareJoined) | { kind: 'pending'; wait: number } | { kind: 'lend'; ticket: number }

/**
 * Réponse de `share_wait` (un appel tenu 25 s au plus) : `waiting` (rappeler), `joined` (l'hôte a accepté : la session
 * s'ouvre), `cancelled` (« Annuler » est passé). Le refus (« L'hôte a refusé. », « L'hôte n'a pas répondu. », « Le
 * relais ne répond pas. ») est l'erreur de la commande.
 */
export type ShareWaitOutcome = { kind: 'waiting' } | ({ kind: 'joined' } & ShareJoined) | { kind: 'cancelled' }

/**
 * Événements `share-role` (le rôle de l'invité de la session `id` a changé), `share-image` (l'image
 * qu'il reçoit : qualité demandée et qualité reçue) et `share-state` (le panneau de l'hôte de la session `id`).
 */
export type ShareRoleEvent = { id: string; role: ShareRole }
export type ShareImageEvent = { id: string; quality: ShareQuality; label: string | null }
export type ShareStateEvent = { id: string; share: Share }

/**
 * Événements `share-request` (une demande à rejoindre arrive pendant que la barre est ouverte sur la session `id` :
 * l'avis « {prénom} demande à rejoindre. », que la barre ne montre pas si son panneau de « Partager » est ouvert) et
 * `share-open-panel` (le choix d'une notification de demande : le panneau de « Partager » de la session s'ouvre ;
 * charge : l'identifiant de la session).
 */
export type ShareRequestEvent = { id: string; text: string; panel?: 'share' | 'lend' }

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

// Prêt de l'ordinateur (KyberNow Share, docs/pret.md, textes de docs/link-ui.md « Prêt de l'ordinateur »).

/**
 * L'état de l'ordinateur d'un ami : ceux du cœur (`free` « Disponible », `busy` « Occupé », `lent` « Prêté à quelqu'un
 * d'autre », `locked` « Verrouillé », `off` « Prêt coupé »), `offline` (« Hors ligne »), `refused` (« Identifiant ou mot
 * de passe refusé. »), `unsupported` (Station trop ancienne), `password` (mot de passe ni retenu ni donné : il est
 * demandé à « Emprunter »), `unknown` (pas encore su).
 */
export type FriendState = 'free' | 'busy' | 'lent' | 'locked' | 'off' | 'offline' | 'refused' | 'unsupported' | 'password' | 'unknown'

/** Le créneau d'un compte d'ami, en secondes Unix : en cours (`now`), et le prochain. */
export type LendSlot = { now: boolean; next: { start: number; end: number } | null }

/**
 * Un ordinateur emprunté (`lend_friends`, événement `lend-friends`) : la carte « Ordinateur de {hôte} » de l'accueil,
 * gardée à part des ordinateurs appairés. `owner` : le prénom de l'hôte (ou le nom de l'ordinateur, ou son adresse).
 */
export type Friend = { id: string; owner: string; computer: string; login: string; remember: boolean; state: FriendState; slot: LendSlot | null; relay: boolean }

/**
 * `lend_invite` : le lien d'invitation, pour l'écran d'ajout. `host` : le prénom de l'hôte qu'il porte (« {hôte} vous
 * prête son ordinateur. ») ; `null` pour un lien d'avant (titre « Avec un compte d'ami »).
 */
export type LendInvite = { login: string; host: string | null; relay: boolean }

/** La session d'emprunt à ouvrir (`lend_connect`) : son titre « Ordinateur de {hôte} » et la teinte de son Admin. */
export type LoanReady = { id: string; title: string; host: string; fingerprint: string | null }

/** Réponse de `lend_borrow` et `lend_come_back` : le prêt démarre, ou « En attente de {hôte}… » (`lend_wait`). */
export type BorrowOutcome = ({ kind: 'ready' } & LoanReady) | { kind: 'pending'; wait: number; host: string }

/** Réponse de `lend_wait` (30 s au plus) ; le refus (« {hôte} a refusé. ») est l'erreur de la commande. */
export type LendWaitOutcome = { kind: 'waiting' } | ({ kind: 'ready' } & LoanReady) | { kind: 'cancelled' }

/** Un préavis de 2 minutes : sa raison, l'heure de la fin (secondes Unix) et son texte (celui du cœur). */
export type LendNotice = { reason: 'taken_back' | 'time_up' | 'slot_end' | 'slot_start'; at: number; text: string }

/** La barre de l'ami qui emprunte (`bar-shown`, événement `lend-borrow`) : « Prêté par {hôte} », la fin (`null` : « Jusqu'à ce que {hôte} reprenne »), le préavis. */
export type BorrowBar = { host: string; until: number | null; notice: LendNotice | null }

/** L'ami qui a l'ordinateur, chez l'hôte. */
export type LendBorrower = { name: string; kind: 'code' | 'friend' | 'slot'; app: string; away: boolean; since: number; until: number | null; steam: 'friend' | 'host'; connected: boolean }

/**
 * Une demande d'emprunt : « {prénom} veut emprunter l'ordinateur. », « Accepter », « Refuser » ; pendant un créneau
 * (`slot`), le texte du créneau et « Accepter » seul (l'ami passe avant l'hôte).
 */
export type LendRequest = { id: string; name: string; kind: 'code' | 'friend' | 'slot'; app: string; away: boolean; since: number }

/**
 * Le code de séance, comme le partage : « 123 456 », le lien des apps (QR code), l'adresse du client web (« Copier le
 * lien »), et son relais (`relay`, comme `Share.relay`).
 */
export type LendCode = { code: string; link: string; web: string; duration: number | null; steam: 'friend' | 'host'; expires: number; relay: RelayState | null; qr: ShareQr | null }

/**
 * Le panneau « Prêter l'ordinateur… » de l'hôte (`bar-shown`, événement `lend-state`, réponses des actions) :
 * `availability` (`unsupported` Station trop ancienne, `unavailable` prêt impossible sur cet ordinateur, `disabled` prêt
 * coupé dans Admin, `unknown` pas encore su), l'état
 * du prêt, l'ami qui a l'ordinateur, le préavis, les demandes et le code de séance.
 */
export type LendPanel = {
  availability: 'available' | 'unsupported' | 'unavailable' | 'disabled' | 'unknown'
  state: 'free' | 'preparing' | 'lent' | 'ending'
  borrower: LendBorrower | null
  notice: LendNotice | null
  requests: LendRequest[]
  code: LendCode | null
}

/** Événements `lend-state` (le panneau de la session d'hôte `id`) et `lend-borrow` (la barre de la session d'emprunt `id`). */
export type LendStateEvent = { id: string; lend: LendPanel }
export type BorrowEvent = { id: string; borrow: BorrowBar }

/** L'accueil de l'hôte pendant un prêt : « Prêté à {prénom} » (jusqu'à `until`), « Reprise dans 1:42 » jusqu'à `notice_at`. */
export type HomeLend = { state: 'preparing' | 'lent' | 'ending'; name: string; until: number | null; notice_at: number | null; notice: LendNotice['reason'] | null }

/**
 * La fin d'une session d'emprunt (dans `session-ended`) : la carte de fin avec le texte du prêt, ou `come_back`
 * (« Revenir », 15 min) ; `left` : l'ami a rendu l'ordinateur, pas de carte mais l'avis « Vous avez rendu l'ordinateur. ».
 */
export type LendEnd = { host: string; text: string | null; come_back: boolean; left: boolean }

/** « Durée » : 1, 2 (proposée) ou 4 heures, ou `null` (« Jusqu'à ce que je reprenne »), en secondes. */
export const lendDurations: (number | null)[] = [3600, 7200, 14400, null]

/** « 18 h », « 18 h 30 » ; “6 PM”, “6:30 PM” : l'heure de ce PC. */
export function clock(at: number) {
  const d = new Date(at * 1000)
  const h = d.getHours()
  const m = d.getMinutes()
  if (i18n.lang === 'fr') return m ? `${h}\u00a0h\u00a0${String(m).padStart(2, '0')}` : `${h}\u00a0h`
  const h12 = h % 12 || 12
  const suffix = h < 12 ? 'AM' : 'PM'
  return m ? `${h12}:${String(m).padStart(2, '0')}\u00a0${suffix}` : `${h12}\u00a0${suffix}`
}

/** « 1:42 » : le compte à rebours d'un préavis (jamais négatif). */
export function countdown(at: number, now = Date.now() / 1000) {
  const s = Math.max(0, Math.ceil(at - now))
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`
}

/** « Encore 1 h 20 » / “1 h 20 left” : le temps restant d'un prêt. */
export const leftText = (until: number, now = Date.now() / 1000) => t('lend.left', { duration: duration(Math.max(1, Math.ceil((until - now) / 60)) * 60) })

/** « Votre créneau : mercredi, 14 h – 18 h » ou, pendant le créneau, « Votre créneau, jusqu'à 18 h ». */
export function slotText(slot: LendSlot | null) {
  if (!slot?.next) return null
  if (slot.now) return t('lend.slot_until', { time: clock(slot.next.end) })
  const day = new Date(slot.next.start * 1000).toLocaleDateString(i18n.lang === 'fr' ? 'fr-FR' : 'en-GB', { weekday: 'long' })
  return t('lend.slot_range', { day, start: clock(slot.next.start), end: clock(slot.next.end) })
}

/** Le texte de l'état d'un ordinateur emprunté (carte de l'accueil) ; `password` : rien à dire. */
export function friendStateText(s: FriendState) {
  switch (s) {
    case 'offline':
      return t('home.offline')
    case 'unknown':
      return t('common.searching')
    case 'refused':
      return t('lend.refused_credentials')
    case 'unsupported':
      return t('lend.unsupported')
    case 'password':
      return ''
    default:
      return t(`lend.state_${s}`)
  }
}

/**
 * Les feuilles du prêt côté ami (LendSheet.svelte) : lien d'invitation, mot de passe à redonner, préparation de
 * l'ordinateur pendant une autre session, attente de l'hôte, carte de fin.
 */
export type LendSheetKind =
  | { kind: 'invite'; link: string; invite: LendInvite }
  | { kind: 'password'; friend: Friend }
  | { kind: 'preparing'; host: string }
  | { kind: 'waiting'; wait: number; host: string }
  | { kind: 'end'; end: LendEnd }
