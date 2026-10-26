// Imitation de l'app pour travailler l'interface dans un navigateur
// (`npm run dev`) : jamais incluse dans l'app (seulement en développement).
// Langue : celle du navigateur, `?lang=en` ou `?lang=fr` pour forcer, ou le
// réglage Langue de l'app (comme dans l'app, sans recharger).

import { i18n, langFromTag, t } from './i18n/index.svelte'
import type { BarComputer, Computer, ConnectInfo, ConnexionView, DisplaysInfo, FondState, Info, KeepSound, LastSession, ScreensEvent, Settings, StationInfo, StationOutcome, StationRestarting, WakeTarget } from './api'

const listeners = new Map<string, ((e: { payload: unknown }) => void)[]>()
const emit = (name: string, payload: unknown) => listeners.get(name)?.forEach((cb) => cb({ payload }))

// Rendus : `?vide` (aucun ordinateur), `?installateur` (la mise à jour de la
// Station passe par l'installateur), `?sansimage` (l'ordinateur n'envoie pas d'image),
// `?redemarrage` (l'image ne vient pas : la Station redémarre, puis la connexion reprend),
// `?sansreveil` (l'ordinateur hors ligne n'a pas de carte réseau connue : « Réessayer »),
// `?echecmaj` (la mise à jour de la Station n'aboutit pas), `?ecrans=3` (ce PC a trois écrans : la
// ligne de l'accueil, la connexion et la barre au pluriel ; la Station de Bureau-Salon en crée quatre),
// `?reprise` (en session, la connexion reprend : écrans changés), `/fond.html` (le fond de la connexion
// d'un autre écran) avec `?sanscreation` (l'ordinateur n'a pas pu créer l'écran : sa ligne).
// Plusieurs ordinateurs : `?ordinateurs=2` (les deux premiers ordinateurs sont en session : sur
// l'accueil, « Afficher la session » ; à 4, « Se connecter » inactif avec la raison ; dans
// `/bar.html`, la barre sert le premier, le menu « Ordinateurs » coche les autres en session, et
// « Garder le son »), `/connexion.html?id=c3` (la fenêtre de connexion d'une session suivante,
// jusqu'à la première image) avec `?echec` (la connexion échoue : la raison et « OK »).
const flags = new URLSearchParams(location.search)
/** Écrans de ce PC imité. */
const deviceScreens = Math.max(1, Number(flags.get('ecrans') ?? 1) || 1)
/** Ordinateurs en session au départ (plusieurs ordinateurs). */
const sessionCount = Math.max(0, Math.min(4, Number(flags.get('ordinateurs') ?? 0) || 0))

const settings = {
  profile: 'bureau',
  codec: null,
  yuv444: null,
  mbps: null,
  reliable: null,
  max_4k: false,
  custom: null,
  displays: 'auto',
  vsync: true,
  mouse: 'auto',
  gamepad: 'xbox360',
  clipboard: true,
  audio: 'auto',
  virtual_display: true,
  battery: 'alleger',
  keep_last_image: true,
  auto_reconnect: true,
}

/** Empreinte d'Admin imitée (32 octets), tirée d'un mot. */
const fp = (seed: string) => {
  let h = 2166136261
  const bytes: string[] = []
  for (let i = 0; i < 32; i++) {
    h = Math.imul(h ^ seed.charCodeAt(i % seed.length) ^ i, 16777619)
    bytes.push((h >>> 24).toString(16).padStart(2, '0'))
  }
  return `sha256:${bytes.join(':')}`
}

const now = Math.floor(Date.now() / 1000)
const station = (version: string, update: Partial<StationInfo['update']> = {}): StationInfo => ({
  version,
  update: { available: false, version: null, installable: false, requires_setup: false, installing: false, ...update },
  restarting: false,
})
/** Une carte réseau imitée pour le réveil (`wake`), dans le /24 de son adresse. */
const card = (mac: string, ipv4: string, kind: WakeTarget['kind'], armed: boolean | null, name: string): WakeTarget => ({
  mac,
  ipv4,
  broadcast: ipv4.replace(/\.\d+$/, '.255'),
  kind,
  armed,
  name,
})
const wakeOf: Record<string, WakeTarget[]> = {
  a1: [card('3C:7C:3F:12:AB:CD', '192.168.1.100', 'ethernet', true, 'Intel(R) Ethernet Connection (7) I219-V')],
  b2: flags.has('sansreveil') ? [] : [card('D8:BB:C1:5A:20:0E', '192.168.1.120', 'ethernet', true, 'Realtek PCIe 2.5GbE Family Controller')],
  c3: [card('A4:B1:C1:00:11:22', '192.168.1.42', 'wifi', false, 'Intel(R) Wi-Fi 6E AX211 160MHz')],
}

// Un ordinateur par état : en ligne avec une dernière session et une Station
// à mettre à jour (compte propre à l'appareil, dit par Admin), hors ligne et
// réveillable, à jour avec une session gardée par la 0.4.x, et une Station
// trop ancienne (sans `station`).
let computers = [
  {
    id: 'a1',
    name: 'Bureau-Salon',
    host: '192.168.1.100',
    port: 9443,
    user: 'PORTABLE-BUREAU-4821',
    // Ajouté par code avant la 0.5.0 : c'est Admin qui le dit (`account.own_device`).
    by_code: false,
    wake: wakeOf.a1,
    fingerprint: fp('bureau-salon'),
    os: 'Windows 11 Pro 24H2',
    gpu: 'NVIDIA GeForce RTX 3080 Ti',
    settings: { ...settings },
    last: {
      median_ms: 8.9,
      p95_ms: 14.2,
      seconds: 2520,
      at: now - 86400 - 3600,
      width: 1920,
      height: 1200,
      refresh: 60,
      // Avec plusieurs écrans : leur nombre, et le débit total.
      ...(deviceScreens > 1 ? { screens: deviceScreens, mbps: 36 * deviceScreens } : { mbps: 36 }),
      codec: 'HEVC',
    } as LastSession | null,
    online: true as boolean | null,
    info: {
      hostname: 'BUREAU-SALON',
      os: 'Windows 11 Pro 24H2',
      gpu: 'NVIDIA GeForce RTX 3080 Ti',
      av1: false,
      admin_version: '0.5.0',
      virtual_display: { available: true, max_screens: 4 },
      station: station('0.5.0', { available: true, version: '0.5.1', installable: !flags.has('installateur'), requires_setup: flags.has('installateur') }),
      wake: wakeOf.a1,
      account: { own_device: true },
    } as Info | null,
    preview: null,
    preview_source: null,
    waking: false,
    station_pending: null as Computer['station_pending'],
  },
  {
    id: 'b2',
    name: 'Atelier',
    host: '192.168.1.120',
    port: 9443,
    user: 'matthieu',
    by_code: false,
    wake: wakeOf.b2,
    fingerprint: fp('atelier'),
    os: 'Windows 10 Pro 22H2',
    gpu: 'AMD Radeon RX 6700 XT',
    settings: { ...settings, profile: 'jeu' },
    last: null as LastSession | null,
    online: false as boolean | null,
    info: null as Info | null,
    preview: null,
    preview_source: null,
    waking: false,
    station_pending: null as Computer['station_pending'],
  },
  {
    id: 'c3',
    name: 'Studio-Photo',
    host: '192.168.1.42',
    port: 31544,
    user: 'matthieu',
    by_code: false,
    wake: wakeOf.c3,
    fingerprint: fp('studio-photo'),
    os: 'Windows 11 Famille 24H2',
    gpu: 'NVIDIA GeForce RTX 4070',
    settings: { ...settings, profile: 'max', mbps: 150 },
    last: { median_ms: 12.4, p95_ms: 21.8, seconds: 640, at: now - 5 * 86400, label: 'Jeu · HEVC' } as LastSession | null,
    online: true as boolean | null,
    info: {
      hostname: 'STUDIO-PHOTO',
      os: 'Windows 11 Famille 24H2',
      gpu: 'NVIDIA GeForce RTX 4070',
      av1: true,
      admin_version: '0.5.1',
      virtual_display: { available: true },
      station: station('0.5.1'),
      wake: wakeOf.c3,
    } as Info | null,
    preview: null,
    preview_source: null,
    waking: false,
    station_pending: null as Computer['station_pending'],
  },
  {
    id: 'd4',
    name: 'Vieux-PC',
    host: 'fe80::1c2b:3aff:fe4d:5e6f',
    port: 9443,
    user: 'admin',
    by_code: false,
    wake: [] as WakeTarget[],
    fingerprint: fp('vieux-pc'),
    os: 'Windows 10 Famille 22H2',
    gpu: 'NVIDIA GeForce GTX 1060',
    settings: { ...settings, profile: 'jeu', virtual_display: false },
    last: null as LastSession | null,
    online: true as boolean | null,
    info: {
      hostname: 'VIEUX-PC',
      os: 'Windows 10 Famille 22H2',
      gpu: 'NVIDIA GeForce GTX 1060',
      av1: false,
      admin_version: '0.4.9',
      virtual_display: { available: false },
      station: null,
      wake: [],
    } as Info | null,
    preview: null,
    preview_source: null,
    waking: false,
    station_pending: null as Computer['station_pending'],
  },
]
if (flags.has('vide')) computers = []
// Plusieurs ordinateurs : un cinquième, pour voir la limite de quatre.
if (flags.has('ordinateurs') && !flags.has('vide')) {
  computers.push({
    ...computers[2],
    id: 'e5',
    name: 'Salle-de-jeux',
    host: '192.168.1.60',
    fingerprint: fp('salle-de-jeux'),
    settings: { ...settings, profile: 'jeu' },
    last: null,
    info: computers[2].info && { ...computers[2].info, hostname: 'SALLE-DE-JEUX' },
  })
}

/** Les ordinateurs en session, dans l'ordre d'ouverture (`?ordinateurs=N` : les N premiers, en ligne). */
let sessions: string[] = computers.slice(0, sessionCount).map((c) => c.id)
for (const c of computers) if (sessions.includes(c.id)) c.online = true
const emitSessions = () => {
  emit('sessions', [...sessions])
  emit('bar-computers', barComputers())
}
/** « Garder le son » des sessions imitées. */
const keepSound = new Set<string>()

/** Le menu « Ordinateurs » de la barre, comme `bar_computer_list` de l'app (`menu::pick`). */
const barComputers = (): BarComputer[] =>
  computers.map((c) => {
    const busy = !!c.station_pending
    const pick: BarComputer['pick'] = sessions.includes(c.id)
      ? 'show'
      : c.online === false && busy
        ? 'busy'
        : c.online === false && c.waking
          ? 'waking'
          : c.online === false && c.wake.length > 0
            ? 'wake'
            : c.online === false
              ? 'retry'
              : sessions.length >= 4
                ? 'full'
                : 'connect'
    const state: BarComputer['state'] = c.online === true ? 'online' : c.online === false ? 'offline' : 'searching'
    const dot: BarComputer['dot'] = c.online === true ? 'ok' : c.online === false ? (c.waking || busy ? 'warn' : 'bad') : 'faint'
    return { id: c.id, name: c.name, state, dot, pick }
  })

/** L'accueil reçoit une copie (comme de l'app, qui envoie du JSON). */
const emitComputers = () => {
  emit('computers', structuredClone(computers))
  emit('bar-computers', barComputers())
}

const thisPc = {
  name: 'PORTABLE-BUREAU',
  gpu: 'Intel(R) Iris(R) Xe Graphics',
  h264: true,
  hevc: true,
  hevc444: true,
  av1: true,
  width: 1920,
  height: 1200,
  refresh: 60,
  audio_channels: 2,
  os: 'Windows 11 Famille 24H2',
  on_battery: false,
}

const discovered = [{ id: 'son', name: 'Studio-Son', host: '192.168.1.77', port: 31544, fingerprint: fp('studio-son') }]

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms))

/** Un ordinateur imité change (Station qui redémarre, s'installe, s'arrête) : l'accueil le voit. */
const change = (id: unknown, f: (c: (typeof computers)[number]) => void) => {
  const c = computers.find((x) => x.id === id)
  if (c) f(c)
  emitComputers()
}

// Notes d'une release : français, `<!-- en -->`, anglais (comme sur GitHub).
const notes = '- Client Windows : première version.\n- Console : accès par code appliqué dès que Kyber est libre.\n<!-- en -->\n- Windows client: first version.\n- Console: code access applied as soon as Kyber is free.'
/** La partie des notes dans la langue de l'interface (comme `release_notes` du cœur). */
const notesFor = (lang: string) => {
  const lines = notes.split('\n')
  const at = lines.findIndex((l) => l.trim() === '<!-- en -->')
  if (at < 0) return notes
  return (lang === 'en' ? lines.slice(at + 1) : lines.slice(0, at)).join('\n').trim()
}
let language: 'auto' | 'fr' | 'en' = 'auto'
let battery: 'alleger' | 'garder' = 'alleger'
let openAtLogin = true
/** Fin du suivi de la Station, comme `follow_loop` de l'app. */
const outcome = (o: StationOutcome) => {
  change(o.id, (c) => (c.station_pending = null))
  emit('station-outcome', o)
}
const update = () => ({ version: '0.5.1', channel: 'stable', notes: notesFor(i18n.lang), page: '', file: 'KyberNow-Link-Setup.exe', size: 1 })

/** Écrans que la session d'un ordinateur imité prend (le calcul du cœur, comme `displays_info`). */
const screensFor = (id: string) => {
  const c = computers.find((x) => x.id === id)
  if (!c) return 1
  const s = c.settings as typeof settings
  const enabled = s.virtual_display && c.info?.virtual_display.available !== false && !s.custom
  const perso = s.codec != null || s.yuv444 != null || s.mbps != null || s.reliable != null
  const all = enabled && (s.displays === 'all' || (s.displays === 'auto' && !perso && (s.profile === 'bureau' || s.profile === 'max')))
  return all ? Math.max(1, Math.min(deviceScreens, c.info?.virtual_display.max_screens ?? 1, 4)) : 1
}

const commands: Record<string, (a: Record<string, unknown>) => unknown> = {
  snapshot: () => ({ computers: structuredClone(computers), discovered: flags.has('vide') ? [] : discovered, this_pc: thisPc, sessions: [...sessions], version: 'dev' }),
  poll_now: async () => {
    await sleep(700)
    emitComputers()
  },
  probe_address: async (a) => {
    await sleep(500)
    return { host: a.host, port: a.port, name: null, fingerprint: 'sha256:9f:3a:c2:71:0d:5e:88:b4' }
  },
  parse_link: (a) => {
    const m = String(a.link).match(/^kybernow:\/\/([^:/?]+):(\d+)/)
    if (!m) throw t('mock.not_a_link')
    return { host: m[1], port: Number(m[2]), name: t('mock.from_link'), fingerprint: 'sha256:aa' }
  },
  pair_start: async () => {
    await sleep(300)
    return '482913'
  },
  pair_poll: () => 'pending',
  pair_cancel: () => null,
  add_computer: async () => {
    await sleep(500)
    throw t('mock.bad_account')
  },
  save_settings: (a) => {
    computers = computers.map((c) => (c.id === a.id ? { ...c, settings: a.settings as typeof settings } : c))
    emitComputers()
  },
  rename_computer: () => null,
  remove_computer: () => null,
  // Rend tout de suite, comme l'app : la session se déroule ensuite (événements). Pendant une
  // session, une session de plus (plusieurs ordinateurs) : en fenêtre, sa connexion dans sa propre
  // fenêtre (imitée seule : `/connexion.html?id=…`), quatre au plus.
  connect: (a) => {
    const id = a.id as string
    if (sessions.includes(id)) return { screens: 0, windowed: true } satisfies ConnectInfo
    if (sessions.length >= 4) throw t('common.four_at_most')
    if (sessions.length > 0) {
      sessions = [...sessions, id]
      emitSessions()
      return { screens: 1, windowed: true } satisfies ConnectInfo
    }
    sessions = [id]
    emitSessions()
    const screens = screensFor(id)
    void (async () => {
      for (const phase of ['preparing', 'connecting', 'first_images']) {
        emit('session-status', { id: a.id, phase, detail: null })
        await sleep(1200)
      }
      // Dernier recours, comme le cœur : l'étape « Connexion », puis l'avis, puis tout reprend.
      if (flags.has('redemarrage')) {
        emit('session-status', { id: a.id, phase: 'connecting', detail: null })
        emit('session-station-restarting', { id: a.id as string } satisfies StationRestarting)
        await sleep(4000)
        for (const phase of ['preparing', 'connecting', 'first_images']) {
          emit('session-status', { id: a.id, phase, detail: null })
          await sleep(1200)
        }
      }
      emit('session-screens', { id: a.id as string, screens } satisfies ScreensEvent)
      emit('session-status', { id: a.id, phase: 'streaming', detail: null })
      // ?sansimage : l'ordinateur n'envoie jamais d'image.
      if (flags.has('sansimage')) return
      await sleep(600)
      emit('session-shown', a.id)
      // ?reprise : la connexion reprend en session (écrans changés), les étapes reviennent, puis l'image.
      if (flags.has('reprise')) {
        await sleep(2000)
        emit('session-replan', { id: a.id as string, screens } satisfies ScreensEvent)
        for (const phase of ['preparing', 'connecting', 'first_images', 'streaming']) {
          emit('session-status', { id: a.id, phase, detail: null })
          await sleep(1200)
        }
        emit('session-shown', a.id)
      }
      // ?ordinateurs : la session reste ouverte (l'accueil s'essaie pendant elle).
      if (flags.has('ordinateurs')) return
      await sleep(1500)
      sessions = sessions.filter((s) => s !== id)
      emit('session-ended', { id: a.id, summary: { error: null, median_ms: 9.1, p95_ms: 13, seconds: 60 } })
      emitSessions()
    })()
    return { screens, windowed: false } satisfies ConnectInfo
  },
  show_session: (a) => {
    if (!sessions.includes(a.id as string)) throw t('mock.no_session')
    console.info(`show_session : la fenêtre de ${a.id} passe devant`)
    return null
  },
  session_keep_sound: (a) => {
    if (a.on) keepSound.add(a.id as string)
    else keepSound.delete(a.id as string)
    emit('session-keep-sound', { id: a.id as string, on: Boolean(a.on) } satisfies KeepSound)
    return null
  },
  bar_computers: () => barComputers(),
  // La fenêtre de connexion d'une session suivante (`/connexion.html?id=c3`) : ses étapes, puis la
  // première image (le passage s'ouvre), ou ?echec : la raison.
  connexion_state: () => {
    const id = new URLSearchParams(location.search).get('id') ?? 'c3'
    void (async () => {
      await sleep(400)
      for (const phase of ['connecting', 'first_images']) {
        emit('session-status', { id, phase, detail: null })
        await sleep(1400)
      }
      if (flags.has('echec')) {
        emit('session-ended', { id, summary: { error: t('mock.connect_refused'), median_ms: null, p95_ms: null, seconds: 4 } })
        return
      }
      emit('session-status', { id, phase: 'streaming', detail: null })
      await sleep(600)
      emit('session-shown', id)
    })()
    return { id, phase: 'preparing', restarting: false, error: null } satisfies ConnexionView
  },
  connexion_close: () => {
    console.info('connexion_close : Annuler, ou OK sous un échec')
    return null
  },
  // Le cœur décide (`Screens::choice`) : inactif, sur « Cet écran », avec une définition sur mesure ou sans
  // écran virtuel (pas demandé, ou pas utilisable sur la Station) ; Auto prend tous les écrans en Bureau
  // et Max, un seul en Jeu et en Personnalisé.
  displays_info: (a) => {
    const s = a.settings as typeof settings
    const c = computers.find((x) => x.id === a.id)
    const virtual_display = s.virtual_display && c?.info?.virtual_display.available !== false
    const enabled = virtual_display && !s.custom
    const shown: Settings['displays'] = enabled ? (s.displays as Settings['displays']) : 'this'
    const max = c?.info?.virtual_display.max_screens ?? 1
    const perso = s.codec != null || s.yuv444 != null || s.mbps != null || s.reliable != null
    const all = shown === 'all' || (shown === 'auto' && !perso && (s.profile === 'bureau' || s.profile === 'max'))
    return { enabled, shown, count: all ? Math.max(1, Math.min(deviceScreens, max, 4)) : 1, virtual_display } satisfies DisplaysInfo
  },
  fond_state: () => ({ not_created: flags.has('sanscreation') }) satisfies FondState,
  session_command: () => null,
  bar_hide: () => null,
  take_action: () => null,
  app_info: () => ({ version: '0.5.0', channel: 'stable', language, lang: i18n.lang, update: update(), arch: 'x64', battery, open_at_login: openAtLogin }),
  set_open_at_login: (a) => (openAtLogin = Boolean(a.on)),
  set_profile: (a) => {
    computers = computers.map((c) => (c.id === a.id ? { ...c, settings: { ...c.settings, profile: a.profile as string, codec: null, yuv444: null, mbps: null, reliable: null } } : c))
    emitComputers()
  },
  set_battery: (a) => {
    battery = a.battery === 'garder' ? 'garder' : 'alleger'
    return battery
  },
  set_language: (a) => {
    language = a.language as typeof language
    const lang = language === 'auto' ? langFromTag(new URLSearchParams(location.search).get('lang') ?? navigator.languages?.[0]) : language
    emit('lang', lang)
    emit('update', { ...update(), notes: notesFor(lang) })
    return lang
  },
  check_update: async () => {
    await sleep(600)
    return null
  },
  set_channel: async () => null,
  bar_menu: () => null,
  bar_width: () => null,
  session_profile: async (a) => {
    // L'app écrit le profil en français, l'interface le traduit (localLabel).
    const names: Record<string, string> = { jeu: 'Jeu', bureau: 'Bureau', max: 'Max' }
    emit('bar-label', { id: a.id, label: `${names[a.profile as string]} · HEVC`, profile: a.profile })
  },
  // La Station : un appareil est « en session » (le premier appel demande de forcer) ;
  // elle redémarre (Admin ne répond plus 5 s), sa mise à jour s'installe (8 s).
  station_restart: async (a) => {
    await sleep(500)
    if (!a.force) throw { kind: 'in_session', message: t('mock.station_in_session'), device: 'Apple TV du salon', seconds: null }
    change(a.id, (c) => ((c.station_pending = 'restart'), c.info?.station && (c.info.station.restarting = true)))
    setTimeout(() => change(a.id, (c) => (c.online = false)), 1500)
    setTimeout(() => change(a.id, (c) => ((c.online = true), c.info?.station && (c.info.station.restarting = false))), 6500)
    setTimeout(() => outcome({ id: a.id as string, outcome: 'restarted', version: null }), 7000)
    return null
  },
  station_update: async (a) => {
    await sleep(1500)
    change(a.id, (c) => ((c.station_pending = 'update'), c.info?.station && (c.info.station.update.installing = true)))
    setTimeout(() => change(a.id, (c) => (c.online = false)), 3000)
    setTimeout(
      () =>
        change(a.id, (c) => {
          c.online = true
          const s = c.info?.station
          if (!s) return
          // ?echecmaj : la Station revient sur l'ancienne version.
          if (flags.has('echecmaj')) {
            s.update.installing = false
            return
          }
          s.version = s.update.version ?? s.version
          s.update = { available: false, version: null, installable: false, requires_setup: false, installing: false }
        }),
      8000,
    )
    setTimeout(() => outcome({ id: a.id as string, outcome: flags.has('echecmaj') ? 'update_failed' : 'updated', version: '0.5.1' }), 8500)
    return '0.5.1'
  },
  // Réveil : le paquet part vers les cartes gardées ; l'ordinateur hors ligne
  // (Atelier) revient en ligne 6 s plus tard, comme après son démarrage.
  wake: async (a) => {
    await sleep(300)
    const c = computers.find((x) => x.id === a.id)
    // Texte du cœur (`wake.no_target`), dans la langue de l'interface.
    if (!c?.wake.length)
      throw i18n.lang === 'en'
        ? "the computer's network card is unknown: the app learns it from KyberNow Station (0.5.0 or later) while the computer is on"
        : "carte réseau de l'ordinateur inconnue : l'app l'apprend de KyberNow Station (0.5.0 ou plus récente) quand l'ordinateur est allumé"
    if (!c.online) {
      change(a.id, (x) => (x.waking = true))
      setTimeout(
        () =>
          change(a.id, (x) => {
            x.waking = false
            x.online = true
            x.info = { hostname: x.name.toUpperCase(), os: x.os, gpu: x.gpu, av1: false, admin_version: '0.5.0', virtual_display: { available: true }, station: station('0.5.0'), wake: x.wake }
          }),
        6000,
      )
    }
    // Diffusion, 255.255.255.255 et adresse directe, ports 9 et 7.
    return c.wake.length * 6
  },
  open_admin: (a) => {
    const c = computers.find((x) => x.id === a.id)
    console.info(`open_admin : https://${c?.host}:${c?.port}/`)
    return null
  },
  install_update: async () => {
    for (let p = 0; p <= 100; p += 10) {
      emit('update-progress', p)
      await sleep(150)
    }
  },
}

export function mockTauri() {
  return {
    core: {
      invoke: async (cmd: string, args: Record<string, unknown> = {}) => {
        const f = commands[cmd]
        if (!f) throw `commande inconnue : ${cmd}`
        return f(args)
      },
    },
    event: {
      listen: async (name: string, cb: (e: { payload: unknown }) => void) => {
        listeners.set(name, [...(listeners.get(name) ?? []), cb])
        // La barre sert la première session (`a1` sans `?ordinateurs`).
        const served = sessions[0] ?? 'a1'
        if (name === 'bar-shown') setTimeout(() => cb({ payload: { id: served, label: 'Jeu · HEVC', profile: 'jeu', multi: deviceScreens > 1, keep_sound: keepSound.has(served) } }), 300)
        if (name === 'stats') {
          const timer = setInterval(() => cb({ payload: { id: served, latency_ms: 8 + Math.random() * 3, fps: 60, width: 1920, height: 1200, decoder: `HEVC · ${t('mock.hardware')}`, relative: false, rtt_ms: 2.1, audio_ms: 18 + Math.random() * 4, audio_gaps: 0, screens: deviceScreens } }), 1000)
          return () => clearInterval(timer)
        }
        return () => {}
      },
    },
    window: { getCurrentWindow: () => ({ minimize: async () => {}, toggleMaximize: async () => {}, close: async () => {} }) },
  }
}
