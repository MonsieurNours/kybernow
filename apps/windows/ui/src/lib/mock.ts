// Imitation de l'app pour travailler l'interface dans un navigateur
// (`npm run dev`) : jamais incluse dans l'app (seulement en développement).
// Langue : celle du navigateur, `?lang=en` ou `?lang=fr` pour forcer, ou le
// réglage Langue de l'app (comme dans l'app, sans recharger).

import { i18n, langFromTag, t } from './i18n/index.svelte'

const listeners = new Map<string, ((e: { payload: unknown }) => void)[]>()
const emit = (name: string, payload: unknown) => listeners.get(name)?.forEach((cb) => cb({ payload }))

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
}

let computers = [
  {
    id: 'a1',
    name: 'Bureau-Salon',
    host: '192.168.1.100',
    port: 9443,
    user: 'matthieu',
    fingerprint: 'sha256:7c:41:e0:9b:aa:bb',
    os: 'Windows 11 Pro 24H2',
    gpu: 'NVIDIA GeForce RTX 3080 Ti',
    settings: { ...settings },
    last: { median_ms: 8.9, p95_ms: 14.2, seconds: 6120, label: 'Bureau · HEVC 4:4:4', at: 0 },
    online: true,
    info: null,
    preview: null,
    preview_source: null,
  },
  {
    id: 'b2',
    name: 'Atelier',
    host: '192.168.1.120',
    port: 9443,
    user: 'matthieu',
    fingerprint: 'sha256:11:22:33:44',
    os: 'Windows 10 Pro 22H2',
    gpu: 'AMD Radeon RX 6700 XT',
    settings: { ...settings, profile: 'jeu' },
    last: null,
    online: false,
    info: null,
    preview: null,
    preview_source: null,
  },
]

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

const discovered = [{ id: 'studio', name: 'Studio-Son', host: '192.168.1.42', port: 31544, fingerprint: 'sha256:aa:bb:cc:dd:ee' }]

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms))

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
const update = () => ({ version: '0.2.9', channel: 'stable', notes: notesFor(i18n.lang), page: '', file: 'KyberNow-Windows-x64.exe', size: 1 })

const commands: Record<string, (a: Record<string, unknown>) => unknown> = {
  snapshot: () => ({ computers, discovered, this_pc: thisPc, session: null, version: 'dev' }),
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
    emit('computers', computers)
  },
  rename_computer: () => null,
  remove_computer: () => null,
  connect: async (a) => {
    for (const phase of ['preparing', 'connecting', 'first_images']) {
      emit('session-status', { id: a.id, phase, detail: null })
      await sleep(1200)
    }
    emit('session-status', { id: a.id, phase: 'streaming', detail: null })
    // ?sansimage : l'ordinateur n'envoie jamais d'image.
    if (new URLSearchParams(location.search).has('sansimage')) return
    await sleep(600)
    emit('session-shown', a.id)
    await sleep(1500)
    emit('session-ended', { id: a.id, summary: { error: null, median_ms: 9.1, p95_ms: 13, seconds: 60 } })
  },
  session_command: () => null,
  bar_hide: () => null,
  take_action: () => null,
  app_info: () => ({ version: '0.2.8', channel: 'stable', language, lang: i18n.lang, update: update(), arch: 'x64' }),
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
          const timer = setInterval(() => cb({ payload: { latency_ms: 8 + Math.random() * 3, fps: 60, width: 1920, height: 1200, decoder: `HEVC · ${t('mock.hardware')}`, relative: false, rtt_ms: 2.1 } }), 1000)
          return () => clearInterval(timer)
        }
        return () => {}
      },
    },
    window: { getCurrentWindow: () => ({ minimize: async () => {}, toggleMaximize: async () => {}, close: async () => {} }) },
  }
}
