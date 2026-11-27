// La capture du micro (docs/micro.md) : getUserMedia, puis l'Opus de WebCodecs (mono, 48 kHz, trames
// de 10 ms, 32 kbit/s, correction d'erreurs intégrée pour 10 % de pertes), chaque paquet donné au
// moteur. Chargée à la demande par la session (micro.svelte.ts), seulement quand le moteur sait
// envoyer le micro : sinon rien de tout cela ne se charge.

import { OPUS_CONFIG } from './micro.svelte'

/** `denied` : le navigateur n'a pas l'autorisation ; `none` : pas de micro ; `failed` : le reste. */
/** `cancelled` : arrêtée par `stop` pendant la demande d'autorisation. */
export type MicStart = 'ok' | 'denied' | 'none' | 'failed' | 'cancelled'

/**
 * Une capture du micro, de `start` à `stop`. `onPacket` reçoit chaque paquet
 * Opus et son heure de capture (µs) ; un paquet vide à l'arrêt dit à
 * l'ordinateur que le micro est coupé.
 */
export class MicCapture {
  private stream: MediaStream | null = null
  private ctx: AudioContext | null = null
  private node: AudioWorkletNode | null = null
  private encoder: AudioEncoder | null = null
  private frames = 0
  private origin = 0
  private running = false

  constructor(private onPacket: (packet: Uint8Array, ptsUs: number) => void) {}

  get active(): boolean {
    return this.running
  }

  async start(deviceId: string, processing: boolean): Promise<MicStart> {
    this.stop(false)
    this.running = true
    try {
      this.stream = await navigator.mediaDevices.getUserMedia({
        audio: {
          // Un micro choisi absent : celui du système, sans perdre le choix (link-ui.md).
          ...(deviceId ? { deviceId: { ideal: deviceId } } : {}),
          echoCancellation: processing,
          noiseSuppression: processing,
          autoGainControl: processing,
          channelCount: 1,
        },
      })
    } catch (e) {
      this.running = false
      const name = (e as DOMException)?.name
      return name === 'NotAllowedError' || name === 'SecurityError' ? 'denied' : name === 'NotFoundError' || name === 'OverconstrainedError' ? 'none' : 'failed'
    }
    if (!this.running) {
      // Arrêtée pendant la demande d'autorisation (la session est partie) : rien n'a échoué (WEB-13).
      this.release()
      return 'cancelled'
    }
    try {
      await this.pipeline()
      return 'ok'
    } catch (e) {
      console.warn('KyberNow : micro non démarré', e)
      this.stop(false)
      return 'failed'
    }
  }

  /** Le micro, à 48 kHz si le navigateur le permet (Firefox refuse un contexte à une autre
   * fréquence que celle du micro : on prend alors la sienne, l'encodeur la ramène). */
  private async pipeline() {
    let ctx = new AudioContext({ sampleRate: 48000, latencyHint: 'interactive' })
    let source: MediaStreamAudioSourceNode
    try {
      source = ctx.createMediaStreamSource(this.stream!)
    } catch {
      await ctx.close().catch(() => {})
      ctx = new AudioContext({ latencyHint: 'interactive' })
      source = ctx.createMediaStreamSource(this.stream!)
    }
    this.ctx = ctx
    const rate = ctx.sampleRate
    const config = { ...OPUS_CONFIG, sampleRate: rate } as AudioEncoderConfig
    if (!(await AudioEncoder.isConfigSupported(config)).supported) throw new Error(`Opus à ${rate} Hz refusé`)
    this.encoder = new AudioEncoder({
      output: (chunk) => {
        const data = new Uint8Array(chunk.byteLength)
        chunk.copyTo(data)
        if (this.running) this.onPacket(data, chunk.timestamp)
      },
      error: (e) => console.warn('KyberNow : encodeur du micro', e),
    })
    this.encoder.configure(config)
    await ctx.audioWorklet.addModule(new URL('./mic-worklet.js', import.meta.url))
    const node = new AudioWorkletNode(ctx, 'kybernow-mic', { numberOfInputs: 1, numberOfOutputs: 0, processorOptions: { rate } })
    this.node = node
    this.frames = 0
    this.origin = Math.round((performance.timeOrigin + performance.now()) * 1000)
    node.port.onmessage = (ev: MessageEvent<Float32Array<ArrayBuffer>>) => {
      const enc = this.encoder
      if (!this.running || !enc || enc.state !== 'configured') return
      const samples = ev.data
      const audio = new AudioData({
        format: 'f32',
        sampleRate: rate,
        numberOfFrames: samples.length,
        numberOfChannels: 1,
        timestamp: this.origin + Math.round((this.frames * 1_000_000) / rate),
        data: samples,
      })
      this.frames += samples.length
      enc.encode(audio)
      audio.close()
    }
    source.connect(node)
    if (ctx.state === 'suspended') await ctx.resume().catch(() => {})
  }

  private release() {
    this.stream?.getTracks().forEach((t) => t.stop())
    this.stream = null
  }

  /** Arrête la capture ; `silence` : dit tout de suite à l'ordinateur que le micro est coupé. */
  stop(silence = true) {
    const was = this.running
    this.running = false
    if (this.node) {
      this.node.port.onmessage = null
      this.node.disconnect()
      this.node = null
    }
    if (this.encoder && this.encoder.state !== 'closed') this.encoder.close()
    this.encoder = null
    this.ctx?.close().catch(() => {})
    this.ctx = null
    this.release()
    if (was && silence) this.onPacket(new Uint8Array(0), Math.round((performance.timeOrigin + performance.now()) * 1000))
  }
}
