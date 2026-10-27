// Moteur factice pour essayer le client web sans Kyber (voir fake-kyber.mjs) :
// mêmes noms que `kyclient_wasm`, il joue la connexion, dessine une image dans la
// toile et note ce que la page lui demande dans `window.__stub.log` (jeton et
// écran de la connexion, entrées reçues, renouvellements du jeton). Les formats
// qu'il dit décoder se changent dans le navigateur :
//   localStorage.setItem('stub.codecs', JSON.stringify([{ codec: 'H264', yuv444: false, hardware_acceleration: 'hardware' }]))
// et `window.__stub.cb(nom, événement)` lui fait jouer un événement (par exemple
// `__stub.cb('stopped', { reason: 'controlplane_closed' })`).

const stub = (window.__stub = window.__stub || { log: [], cb: null })
const note = (kind, data = {}) => stub.log.push({ t: Math.round(performance.now()), kind, ...data })

/** Les revendications d'un jeton (rôle, écran d'un invité). */
function claims(token) {
  try {
    return JSON.parse(atob(token.split('.')[1].replace(/-/g, '+').replace(/_/g, '/')))
  } catch {
    return { raw: String(token).slice(0, 12) }
  }
}

export default async function init() {
  note('init')
}

export function supportedVideoCodecs() {
  let list = null
  try {
    list = JSON.parse(localStorage.getItem('stub.codecs') || 'null')
  } catch {
    // Liste illisible : celle par défaut.
  }
  return (
    list ?? [
      { codec: 'H264', yuv444: false, hardware_acceleration: 'prefer-hardware' },
      { codec: 'HEVC', yuv444: false, hardware_acceleration: 'prefer-hardware' },
    ]
  )
}

export class StreamingConfigBuilder {
  build() {
    const cfg = { ...this }
    cfg.clone = () => ({ ...cfg })
    return cfg
  }
}

export class AuthCredentials {
  static jwt(token) {
    return { token }
  }
}

export class Client {
  static async new(host, port, credentials, cb, _a, _b, frameCb) {
    const c = new Client()
    c.cb = cb
    c.frameCb = frameCb
    c.token = credentials.token
    stub.cb = (name, event) => cb(name, event)
    note('client-new', { host, port, claims: claims(credentials.token) })
    // Ce que le vrai moteur écoute : le clavier sur le document, les boutons sur la fenêtre.
    c.onKey = (e) => note('input-key', { code: e.code })
    c.onDown = (e) => note('input-mousedown', { button: e.button })
    c.onUp = (e) => note('input-mouseup', { button: e.button })
    document.addEventListener('keydown', c.onKey)
    window.addEventListener('mousedown', c.onDown)
    window.addEventListener('mouseup', c.onUp)
    return c
  }

  connect() {
    note('connect', { claims: claims(this.token) })
    setTimeout(() => this.cb('controlplane_connected', { displays: [{ id: stub.display ?? 2221634456, name: 'DISPLAY1', width: 1920, height: 1080 }] }), 250)
  }

  async start_streaming(canvasId, cfg) {
    note('start-streaming', { display_index: cfg.display_index, codec: cfg.codec, yuv444: cfg.yuv444, clipboard: cfg.clipboard, inputs: cfg.inputs, protocol: cfg.protocol })
    const canvas = document.getElementById(canvasId)
    this.canvas = canvas
    canvas.width = 1920
    canvas.height = 1080
    const ctx = canvas.getContext('2d')
    let n = 0
    this.draw = setInterval(() => {
      n++
      const g = ctx.createLinearGradient(0, 0, 1920, 1080)
      g.addColorStop(0, '#1b2a41')
      g.addColorStop(1, '#324a5f')
      ctx.fillStyle = g
      ctx.fillRect(0, 0, 1920, 1080)
      ctx.fillStyle = '#c9f2da'
      ctx.font = '64px sans-serif'
      ctx.fillText(`Image factice · ${claims(this.token).kn_role ?? 'hôte'} · ${n % 1000}`, 120, 200)
      ctx.fillStyle = '#9a8cff'
      ctx.fillRect(120 + ((n * 6) % 1500), 400, 200, 120)
      const now = performance.now() * 1000
      this.frameCb({ acquired: now, sent: now + 3000, received: now + 5200, displayed: now + 9000, prepared: now + 9000 })
    }, 33)
    this.onMove = () => note('input-mousemove')
    this.onWheel = () => note('input-wheel')
    canvas.addEventListener('mousemove', this.onMove)
    canvas.addEventListener('wheel', this.onWheel)
    setTimeout(() => {
      this.cb('streaming_started', {})
      this.cb('canvas_resized', { width: 1920, height: 1080 })
    }, 300)
  }

  async stop_streaming() {
    note('stop-streaming')
    clearInterval(this.draw)
    setTimeout(() => this.cb('streaming_stopped', {}), 50)
  }

  update_credentials(creds) {
    this.token = creds.token
    note('update-credentials', { claims: claims(creds.token) })
  }

  async disconnect_and_stop() {
    note('disconnect-and-stop')
    clearInterval(this.draw)
    document.removeEventListener('keydown', this.onKey)
    window.removeEventListener('mousedown', this.onDown)
    window.removeEventListener('mouseup', this.onUp)
    this.canvas?.removeEventListener('mousemove', this.onMove)
    this.canvas?.removeEventListener('wheel', this.onWheel)
    setTimeout(() => this.cb('stopped', { reason: 'requested' }), 10)
  }

  audio_set_mute(m) {
    note('mute', { m })
  }

  async audio_try_resume() {
    return 1
  }

  async audio_resume() {}

  send_ctrl_alt_del() {
    note('ctrl-alt-del')
  }
}
