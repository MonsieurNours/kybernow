// Le moteur web de Kyber (kyclient_wasm), servi par la console à côté de la
// page (/ecran/kyclient_wasm.js, relayé depuis le /webclient/ de Kyber).
// On n'en garde que l'essentiel : se connecter avec le jeton de la console,
// démarrer l'image dans une toile, mesurer, s'arrêter.

export interface CodecSupport {
  codec: string
  yuv444: boolean
  hardware: boolean
}

export interface StreamSettings {
  /** kymux : WebTransport (QUIC) ; websocket : par la console, quand WebTransport ne marche pas. */
  protocol: 'kymux' | 'websocket'
  codec: 'h264' | 'hevc'
  yuv444: boolean
  bitrate: number
  reliable: boolean
  audio: boolean
  clipboard: boolean
}

export interface Latency {
  /** Médiane de bout en bout, en ms. */
  total: number
  server: number
  network: number
  client: number
}

export type EngineEvent =
  | { type: 'connecting' }
  /** Plan de contrôle connecté : Kyber a accepté la session, l'image n'a pas encore commencé. */
  | { type: 'connected' }
  | { type: 'reconnecting'; attempt: number; max: number }
  | { type: 'auth-required' }
  /** Kyber a démarré l'image (`streaming_started`) : les données doivent arriver. */
  | { type: 'streaming' }
  | { type: 'first-image' }
  | { type: 'resized'; width: number; height: number }
  | { type: 'stopped'; reason: string }
  | { type: 'error'; message: string }

type Wasm = any

let modulePromise: Promise<Wasm> | null = null

/** Charge le moteur une fois (fichier JS + WebAssembly). */
export function loadEngine(): Promise<Wasm> {
  if (!modulePromise) {
    const url = new URL('kyclient_wasm.js', new URL('/ecran/', location.origin)).href
    modulePromise = import(/* @vite-ignore */ url).then(async (m) => {
      await m.default()
      return m
    })
    modulePromise.catch(() => (modulePromise = null))
  }
  return modulePromise
}

/** Formats que ce navigateur décode (et s'il le fait par la carte graphique). */
export async function supportedCodecs(): Promise<CodecSupport[]> {
  const m = await loadEngine()
  const list = await m.supportedVideoCodecs()
  return (list as any[]).map((c) => ({
    codec: String(c.codec).toLowerCase(),
    yuv444: !!c.yuv444,
    hardware: String(c.hardware_acceleration ?? '').toLowerCase().includes('hardware'),
  }))
}

/** Médianes glissantes sur les ~3 dernières secondes d'images. */
class LatencyWindow {
  private samples: { t: number; total: number; server: number; network: number; client: number }[] = []
  frames = 0

  push(frame: any) {
    const { acquired, sent, received, displayed, prepared } = frame
    const end = displayed ?? prepared
    if (![acquired, sent, received, end].every((v) => typeof v === 'number' && Number.isFinite(v) && v > 0)) return
    const server = (sent - acquired) / 1000
    const network = (received - sent) / 1000
    const client = (end - received) / 1000
    const total = server + network + client
    if (total <= 0 || total > 5000) return
    const t = performance.now()
    this.samples.push({ t, total, server, network, client })
    this.frames++
    while (this.samples.length && t - this.samples[0].t > 3000) this.samples.shift()
  }

  /** Images reçues par seconde, sur la fenêtre. */
  fps(): number {
    if (this.samples.length < 2) return 0
    const span = (this.samples[this.samples.length - 1].t - this.samples[0].t) / 1000
    return span > 0 ? Math.round((this.samples.length - 1) / span) : 0
  }

  median(): Latency | null {
    if (this.samples.length < 5) return null
    const med = (k: 'total' | 'server' | 'network' | 'client') => {
      const v = this.samples.map((s) => s[k]).sort((a, b) => a - b)
      return v[Math.floor(v.length / 2)]
    }
    return { total: med('total'), server: med('server'), network: med('network'), client: med('client') }
  }
}

/** Moteurs en cours d'arrêt, toutes sessions confondues, et ce qui doit
 * suivre leur arrêt (rendre l'écran à la console). */
let retiring: Promise<unknown> = Promise.resolve()

/** Arrête un moteur, puis lance `after`. Le prochain moteur, ou la prochaine
 * préparation d'écran, attend `allStopped()` : jamais deux moteurs à la fois
 * (chacun écoute souris et clavier sur la page), jamais une fin de session
 * qui arrive après la préparation de la suivante. */
export function retire(e: Engine | null, after?: () => Promise<unknown>): Promise<unknown> {
  const stop = e ? e.stop() : Promise.resolve()
  retiring = Promise.all([retiring, stop])
    .then(() => after?.())
    .catch(() => {})
  return retiring
}

export function allStopped(): Promise<unknown> {
  return retiring
}

export class Engine {
  private m: Wasm = null
  private client: any = null
  private config: any = null
  private stopping = false
  /** Fin de l'arrêt : l'événement `stopped` du moteur. */
  private stoppedNow: (() => void) | null = null
  private stopped = new Promise<void>((r) => (this.stoppedNow = r))
  private gotImage = false
  readonly latency = new LatencyWindow()
  /** Médianes de toute la session (pour la console, à la fin). */
  private sessionTotals: number[] = []

  constructor(
    private canvasId: string,
    private onEvent: (e: EngineEvent) => void,
  ) {}

  async connect(host: string, port: string, token: string, s: StreamSettings) {
    const m = await loadEngine()
    this.m = m
    const b = new m.StreamingConfigBuilder()
    b.protocol = s.protocol
    if (s.protocol === 'kymux') {
      b.kymux_video = s.reliable ? 'reliable' : 'unreliable_fec'
      b.kymux_audio = 'reliable'
    }
    b.bitrate = s.bitrate
    b.codec = s.codec
    b.yuv444 = s.yuv444
    b.audio = s.audio
    b.inputs = true
    b.clipboard = s.clipboard
    b.intra_refresh = true
    b.display_index = 0
    b.video_buffer = 0
    this.config = b.build()

    const credentials = m.AuthCredentials.jwt(token)
    this.onEvent({ type: 'connecting' })
    let client: any
    try {
      client = await m.Client.new(
        host,
        port,
        credentials,
        (name: string, event: any) => this.handle(name, event),
        true,
        'live',
        (frame: any) => {
          this.latency.push(frame)
        },
        true,
      )
    } catch (e) {
      this.stoppedNow?.()
      throw e
    }
    // Arrêté pendant sa création : il ne doit pas démarrer.
    if (this.stopping) {
      client.stop().catch(() => {})
      this.stoppedNow?.()
      return
    }
    this.client = client
    client.connect()
  }

  private async handle(name: string, event: any) {
    if (name === 'stopped') this.stoppedNow?.()
    // Un moteur qu'on arrête ne parle plus (bascule de transport, fin).
    if (this.stopping) return
    switch (name) {
      case 'controlplane_connected':
        this.onEvent({ type: 'connected' })
        try {
          await this.client.start_streaming(this.canvasId, this.config.clone())
          // `streaming_started` le dit aussi ; la page n'en tient compte qu'une fois.
          if (!this.stopping) this.onEvent({ type: 'streaming' })
        } catch (e) {
          if (!this.stopping) this.onEvent({ type: 'error', message: `start_streaming: ${e}` })
        }
        break
      case 'streaming_started':
        this.onEvent({ type: 'streaming' })
        break
      case 'streaming_start_failed':
        this.onEvent({ type: 'error', message: 'streaming_start_failed' })
        break
      case 'controlplane_connect_failed':
        this.onEvent({ type: 'error', message: String(event) })
        break
      case 'reconnecting':
        this.onEvent({ type: 'reconnecting', attempt: event.attempt, max: event.maxAttempts })
        break
      case 'authentication_required':
        // Jeton refusé : le moteur attend un nouveau jeton puis `connect()`.
        this.onEvent({ type: 'auth-required' })
        break
      case 'reconnected':
        if (event.shouldRestartStreaming) this.onEvent({ type: 'stopped', reason: 'restart' })
        break
      case 'canvas_resized':
        this.onEvent({ type: 'resized', width: event.width, height: event.height })
        if (!this.gotImage) {
          this.gotImage = true
          this.onEvent({ type: 'first-image' })
        }
        break
      case 'image_decoded':
        if (!this.gotImage) {
          this.gotImage = true
          this.onEvent({ type: 'first-image' })
        }
        break
      case 'streaming_stopped':
        if (!this.stopping) this.onEvent({ type: 'stopped', reason: 'streaming_stopped' })
        break
      case 'stopped': {
        const reason = typeof event?.reason === 'function' ? event.reason() : String(event ?? '')
        this.client = null
        if (!this.stopping) this.onEvent({ type: 'stopped', reason })
        break
      }
    }
  }

  /** Médiane de bout en bout des dernières secondes (et mémorisée pour la fin). */
  sample(): Latency | null {
    const l = this.latency.median()
    if (l) this.sessionTotals.push(l.total)
    return l
  }

  sessionMedian(): number | null {
    if (!this.sessionTotals.length) return null
    const v = [...this.sessionTotals].sort((a, b) => a - b)
    return v[Math.floor(v.length / 2)]
  }

  /** Nouveau jeton (la console en signe pour 12 h) : servira aux reconnexions.
   * `resume` : le moteur attendait ce jeton pour retenter la connexion. */
  updateCredentials(token: string, resume = false) {
    if (!this.client || !this.m) return
    try {
      this.client.update_credentials(this.m.AuthCredentials.jwt(token))
      if (resume) this.client.connect()
    } catch (e) {
      console.warn('KyberNow : jeton non transmis au moteur', e)
    }
  }

  ctrlAltDel() {
    this.client?.send_ctrl_alt_del()
  }

  setMute(mute: boolean) {
    this.client?.audio_set_mute(mute)
  }

  resumeAudio() {
    this.client?.audio_try_resume?.()
  }

  /** Arrête le moteur et attend son événement `stopped` (ses écouteurs de
   * souris et de clavier sont alors retirés) : un nouveau moteur créé avant
   * doublerait les entrées. Un moteur bloqué (connexion qui n'aboutit pas)
   * peut ne jamais répondre : `timeout` borne l'attente. */
  stop(timeout = 3000): Promise<void> {
    if (!this.stopping) {
      this.stopping = true
      const c = this.client
      this.client = null
      if (c) {
        // `disconnect_and_stop` consomme le client : en cas d'échec, il n'y a
        // plus rien à arrêter.
        try {
          c.disconnect_and_stop(true).catch(() => this.stoppedNow?.())
        } catch {
          this.stoppedNow?.()
        }
      } else if (!this.m) {
        // Jamais créé : rien à attendre.
        this.stoppedNow?.()
      }
    }
    return Promise.race([this.stopped, new Promise<void>((r) => setTimeout(r, timeout))])
  }
}
