// Imitation de l'app pour travailler l'interface dans un navigateur
// (`npm run dev`) : jamais incluse dans l'app (seulement en développement).
// Langue : celle du navigateur, `?lang=en` ou `?lang=fr` pour forcer, ou le
// réglage Langue de l'app (comme dans l'app, sans recharger).

import { i18n, langFromTag, t } from './i18n/index.svelte'
import type { Info, LastSession, StationInfo, StationRestarting, WakeTarget } from './api'

const listeners = new Map<string, ((e: { payload: unknown }) => void)[]>()
const emit = (name: string, payload: unknown) => listeners.get(name)?.forEach((cb) => cb({ payload }))

// Rendus : `?vide` (aucun ordinateur), `?installateur` (la mise à jour de la
// Station passe par l'installateur), `?sansimage` (l'ordinateur n'envoie pas d'image),
// `?redemarrage` (l'image ne vient pas : la Station redémarre, puis la connexion reprend),
// `?sansreveil` (l'ordinateur hors ligne n'a pas de carte réseau connue : « Réessayer »),
// `?echecmaj` (la mise à jour de la Station n'aboutit pas).
const flags = new URLSearchParams(location.search)

const settings = {
  profile: 'bureau',
  codec: null,
  yuv444: null,
  mbps: null,
  reliable: null,
  max_4k: false,
  custom: null,
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
    last: { median_ms: 8.9, p95_ms: 14.2, seconds: 2520, at: now - 86400 - 3600, width: 1920, height: 1200, refresh: 60, codec: 'HEVC', mbps: 36 } as LastSession | null,
    online: true as boolean | null,
    info: {
      hostname: 'BUREAU-SALON',
      os: 'Windows 11 Pro 24H2',
      gpu: 'NVIDIA GeForce RTX 3080 Ti',
      av1: false,
      admin_version: '0.5.0',
      virtual_display: { available: true },
      station: station('0.5.0', { available: true, version: '0.5.1', installable: !flags.has('installateur'), requires_setup: flags.has('installateur') }),
      wake: wakeOf.a1,
      account: { own_device: true },
    } as Info | null,
    preview: null,
    preview_source: null,
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
  },
]
if (flags.has('vide')) computers = []

/** L'accueil reçoit une copie (comme de l'app, qui envoie du JSON). */
const emitComputers = () => emit('computers', structuredClone(computers))

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
const update = () => ({ version: '0.5.1', channel: 'stable', notes: notesFor(i18n.lang), page: '', file: 'KyberNow-Link-Setup.exe', size: 1 })

const commands: Record<string, (a: Record<string, unknown>) => unknown> = {
  snapshot: () => ({ computers: structuredClone(computers), discovered: flags.has('vide') ? [] : discovered, this_pc: thisPc, session: null, version: 'dev' }),
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
  connect: async (a) => {
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
    emit('session-status', { id: a.id, phase: 'streaming', detail: null })
    // ?sansimage : l'ordinateur n'envoie jamais d'image.
    if (flags.has('sansimage')) return
    await sleep(600)
    emit('session-shown', a.id)
    await sleep(1500)
    emit('session-ended', { id: a.id, summary: { error: null, median_ms: 9.1, p95_ms: 13, seconds: 60 } })
  },
  session_command: () => null,
  bar_hide: () => null,
  take_action: () => null,
  app_info: () => ({ version: '0.5.0', channel: 'stable', language, lang: i18n.lang, update: update(), arch: 'x64', battery }),
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
  session_profile: async (a) => {
    // L'app écrit le profil en français, l'interface le traduit (localLabel).
    const names: Record<string, string> = { jeu: 'Jeu', bureau: 'Bureau', max: 'Max' }
    emit('bar-label', { label: `${names[a.profile as string]} · HEVC`, profile: a.profile })
  },
  // La Station : un appareil est « en session » (le premier appel demande de forcer) ;
  // elle redémarre (Admin ne répond plus 5 s), sa mise à jour s'installe (8 s).
  station_restart: async (a) => {
    await sleep(500)
    if (!a.force) throw { kind: 'in_session', message: t('mock.station_in_session'), device: 'Apple TV du salon', seconds: null }
    change(a.id, (c) => c.info?.station && (c.info.station.restarting = true))
    setTimeout(() => change(a.id, (c) => (c.online = false)), 1500)
    setTimeout(() => change(a.id, (c) => ((c.online = true), c.info?.station && (c.info.station.restarting = false))), 6500)
    return null
  },
  station_update: async (a) => {
    await sleep(1500)
    change(a.id, (c) => c.info?.station && (c.info.station.update.installing = true))
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
    if (!c.online)
      setTimeout(
        () =>
          change(a.id, (x) => {
            x.online = true
            x.info = { hostname: x.name.toUpperCase(), os: x.os, gpu: x.gpu, av1: false, admin_version: '0.5.0', virtual_display: { available: true }, station: station('0.5.0'), wake: x.wake }
          }),
        6000,
      )
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
        if (name === 'bar-shown') setTimeout(() => cb({ payload: { label: 'Jeu · HEVC', profile: 'jeu' } }), 300)
        if (name === 'stats') {
          const timer = setInterval(() => cb({ payload: { latency_ms: 8 + Math.random() * 3, fps: 60, width: 1920, height: 1200, decoder: `HEVC · ${t('mock.hardware')}`, relative: false, rtt_ms: 2.1, audio_ms: 18 + Math.random() * 4, audio_gaps: 0 } }), 1000)
          return () => clearInterval(timer)
        }
        return () => {}
      },
    },
    window: { getCurrentWindow: () => ({ minimize: async () => {}, toggleMaximize: async () => {}, close: async () => {} }) },
  }
}
