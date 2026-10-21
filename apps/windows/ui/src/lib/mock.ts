// Imitation de l'app pour travailler l'interface dans un navigateur
// (`npm run dev`) : jamais incluse dans l'app (seulement en développement).

const listeners = new Map<string, ((e: { payload: unknown }) => void)[]>()
const emit = (name: string, payload: unknown) => listeners.get(name)?.forEach((cb) => cb({ payload }))

const settings = {
  profile: 'bureau',
  codec: null,
  yuv444: null,
  mbps: null,
  reliable: null,
  max_4k: false,
  vsync: true,
  mouse: 'auto',
  gamepad: 'xbox360',
  clipboard: true,
  audio: 'auto',
  virtual_display: true,
  battery: 'alleger',
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

const commands: Record<string, (a: Record<string, unknown>) => unknown> = {
  snapshot: () => ({ computers, discovered, this_pc: thisPc, session: null, version: 'dev' }),
  probe_address: async (a) => {
    await sleep(500)
    return { host: a.host, port: a.port, name: null, fingerprint: 'sha256:9f:3a:c2:71:0d:5e:88:b4' }
  },
  parse_link: (a) => {
    const m = String(a.link).match(/^kybernow:\/\/([^:/?]+):(\d+)/)
    if (!m) throw "ce n'est pas un lien kybernow://"
    return { host: m[1], port: Number(m[2]), name: 'Depuis le lien', fingerprint: 'sha256:aa' }
  },
  pair_start: async () => {
    await sleep(300)
    return '482913'
  },
  pair_poll: () => 'pending',
  pair_cancel: () => null,
  add_computer: async () => {
    await sleep(500)
    throw 'console : compte ou mot de passe incorrect (401)'
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
    await sleep(1500)
    emit('session-ended', { id: a.id, summary: { error: null, median_ms: 9.1, p95_ms: 13, seconds: 60 } })
  },
  session_command: () => null,
  bar_hide: () => null,
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
        if (name === 'stats') {
          const t = setInterval(() => cb({ payload: { latency_ms: 8 + Math.random() * 3, fps: 60, width: 1920, height: 1200, decoder: 'HEVC · carte graphique', relative: false, rtt_ms: 2.1 } }), 1000)
          return () => clearInterval(t)
        }
        return () => {}
      },
    },
    window: { getCurrentWindow: () => ({ minimize: async () => {}, toggleMaximize: async () => {}, close: async () => {} }) },
  }
}
